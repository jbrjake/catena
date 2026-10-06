//! The line rules over the published crates' `src/`, and the file-length rule.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::mask::{mask, test_lines};
use super::{CRATES, Report, Rule, Severity, is_seed_staged, read, rel, walk};

/// Files above this many lines get a warning.
pub(crate) const LENGTH_TARGET: usize = 500;
/// Files above this many lines fail.
pub(crate) const LENGTH_CEILING: usize = 750;

/// What a rule needs to know about the file it is looking at.
struct SourceFile<'a> {
    krate: &'a str,
    /// Under the crate's `src/bin/`: host code, which may read the environment.
    in_bin: bool,
    is_fmath: bool,
    /// A `tests.rs` or `*_tests.rs` file, attached under `#[cfg(test)]`.
    is_test_file: bool,
}

struct LineRule {
    rule: Rule,
    pattern: &'static LazyLock<Regex>,
    applies: fn(&SourceFile<'_>) -> bool,
    /// Test-only code is exempt.
    skip_tests: bool,
    /// Extra test on a match's first capture group, for rules a regex alone can't state.
    capture_check: Option<fn(&str) -> bool>,
    message: &'static str,
}

static TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bInstant\b|\bSystemTime\b").expect("valid regex"));
static RNG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:thread_rng|from_entropy)\b|\brand::").expect("valid regex"));
const TRANSCENDENTALS: &str = "sin|cos|tan|asin|acos|atan|atan2|sinh|cosh|tanh|asinh|acosh|\
     atanh|sin_cos|exp|exp2|exp_m1|ln|ln_1p|log|log2|log10|powf|powi|hypot|cbrt";
static FMATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\.\s*(?:{TRANSCENDENTALS})\s*\(|\b(?:f32|f64)\s*::\s*(?:{TRANSCENDENTALS})\b"
    ))
    .expect("valid regex")
});
static FLOAT_ORDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"partial_cmp\s*\(.*\)\s*\.\s*(?:unwrap|expect)\b").expect("valid regex")
});
static DISPLAY_WIDTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\.\s*len\s*\(\s*\)").expect("valid regex")
});
static UNWRAP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.\s*unwrap\s*\(\s*\)").expect("valid regex"));
static IO_FENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bstd\s*::\s*(?:env|fs|io|net|process)\b|\b(?:eprintln|println|eprint|print|dbg)!")
        .expect("valid regex")
});

/// Whether the receiver of a `.len()` is display text: its last `_`-separated word is
/// `label`, `title`, `name` or `symbol`. Plurals (`labels.len()`) count items and pass, as do
/// words that merely contain one (`filename`).
fn is_display_text(ident: &str) -> bool {
    let last = ident
        .rsplit('_')
        .next()
        .unwrap_or(ident)
        .to_ascii_lowercase();
    matches!(last.as_str(), "label" | "title" | "name" | "symbol")
}

fn everywhere(_: &SourceFile<'_>) -> bool {
    true
}

fn core_outside_fmath(file: &SourceFile<'_>) -> bool {
    file.krate == "catena" && !file.is_fmath
}

fn fenced(file: &SourceFile<'_>) -> bool {
    (file.krate == "catena" || file.krate == "catena-ratatui") && !file.in_bin
}

static RULES: [LineRule; 7] = [
    LineRule {
        rule: Rule::Time,
        pattern: &TIME,
        applies: everywhere,
        skip_tests: false,
        capture_check: None,
        message: "clock type: time enters only through tick(dt) (plan §10.4)",
    },
    LineRule {
        rule: Rule::Rng,
        pattern: &RNG,
        applies: everywhere,
        skip_tests: false,
        capture_check: None,
        message: "randomness: the library draws no random numbers (plan §8.1)",
    },
    LineRule {
        rule: Rule::Fmath,
        pattern: &FMATH,
        applies: core_outside_fmath,
        skip_tests: false,
        capture_check: None,
        message: "std transcendental: call it through catena's fmath module (plan §11)",
    },
    LineRule {
        rule: Rule::FloatOrder,
        pattern: &FLOAT_ORDER,
        applies: everywhere,
        skip_tests: false,
        capture_check: None,
        message: "float ordering goes through total_cmp (plan §11)",
    },
    LineRule {
        rule: Rule::DisplayWidth,
        pattern: &DISPLAY_WIDTH,
        applies: everywhere,
        skip_tests: false,
        capture_check: Some(is_display_text),
        message: ".len() on display text counts bytes: measure display columns with \
                  unicode-width (plan §5)",
    },
    LineRule {
        rule: Rule::Unwrap,
        pattern: &UNWRAP,
        applies: everywhere,
        skip_tests: true,
        capture_check: None,
        message: "unwrap() outside tests: use ? or expect with the invariant",
    },
    LineRule {
        rule: Rule::IoFence,
        pattern: &IO_FENCE,
        applies: fenced,
        skip_tests: false,
        capture_check: None,
        message: "I/O, environment and printing are fenced out of the library (plan §2.2)",
    },
];

static ALLOW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"//\s*lint-allow:\s*(.*)$").expect("valid regex"));

/// A `// lint-allow:` comment on one line.
enum Allow {
    Valid(Rule),
    Malformed,
}

/// Parses `// lint-allow: <rule> — <why>` (the separator may also be `--`, `-` or `:`). A
/// known rule and a non-empty reason are both required.
fn parse_allow(raw_line: &str) -> Option<Allow> {
    let rest = ALLOW.captures(raw_line)?.get(1)?.as_str().trim();
    let (name, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let after = after.trim_start();
    let reason = ["—", "--", "-", ":"]
        .iter()
        .find_map(|sep| after.strip_prefix(sep))
        .map(str::trim);
    match (Rule::from_name(name), reason) {
        (Some(rule), Some(why)) if !why.is_empty() => Some(Allow::Valid(rule)),
        _ => Some(Allow::Malformed),
    }
}

/// Runs the line rules and the length rule.
pub(crate) fn check(root: &Path, report: &mut Report) -> Result<()> {
    for krate in CRATES {
        for path in walk(&root.join(krate).join("src"))? {
            if path.extension().is_some_and(|e| e == "rs") && !is_seed_staged(&path) {
                check_file(root, krate, &path, report)?;
            }
        }
    }
    let mut length_roots: Vec<_> = CRATES
        .iter()
        .flat_map(|k| ["src", "tests", "benches"].map(|d| root.join(k).join(d)))
        .collect();
    length_roots.push(root.join("xtask").join("src"));
    for dir in length_roots {
        for path in walk(&dir)? {
            if path.extension().is_some_and(|e| e == "rs") && !is_seed_staged(&path) {
                check_length(&rel(root, &path), &read(&path)?, report);
            }
        }
    }
    Ok(())
}

fn check_length(rel_path: &str, text: &str, report: &mut Report) {
    let lines = text.lines().count();
    if lines > LENGTH_CEILING {
        let message = format!("{lines} lines, over the {LENGTH_CEILING}-line ceiling");
        report.push(Rule::FileLength, Severity::Error, rel_path, 0, message);
    } else if lines > LENGTH_TARGET {
        let message = format!("{lines} lines, over the {LENGTH_TARGET}-line target");
        report.push(Rule::FileLength, Severity::Warning, rel_path, 0, message);
    }
}

fn check_file(root: &Path, krate: &str, path: &Path, report: &mut Report) -> Result<()> {
    let rel_path = rel(root, path);
    let in_src = |suffix: &str| rel_path.starts_with(&format!("{krate}/src/{suffix}"));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let file = SourceFile {
        krate,
        in_bin: in_src("bin/"),
        is_fmath: krate == "catena" && rel_path == "catena/src/fmath.rs",
        is_test_file: stem == "tests" || stem.ends_with("_tests"),
    };
    let raw = read(path)?;
    let masked = mask(&raw);
    let tests = test_lines(&masked);
    report.files_scanned += 1;
    for (idx, (code, raw_line)) in masked.split('\n').zip(raw.split('\n')).enumerate() {
        let line_no = idx + 1;
        let allow = parse_allow(raw_line);
        let mut allow_used = false;
        for rule in RULES.iter().filter(|r| (r.applies)(&file)) {
            if rule.skip_tests && (file.is_test_file || tests[idx]) {
                continue;
            }
            let hit = rule.pattern.captures_iter(code).any(|caps| {
                rule.capture_check
                    .is_none_or(|check| caps.get(1).is_some_and(|m| check(m.as_str())))
            });
            if !hit {
                continue;
            }
            if matches!(allow, Some(Allow::Valid(r)) if r == rule.rule) {
                allow_used = true;
                *report.allowances.entry(rule.rule.name()).or_default() += 1;
            } else {
                report.push(
                    rule.rule,
                    Severity::Error,
                    &rel_path,
                    line_no,
                    rule.message.into(),
                );
            }
        }
        match allow {
            Some(Allow::Malformed) => report.push(
                Rule::LintAllow,
                Severity::Error,
                &rel_path,
                line_no,
                "malformed lint-allow: write `// lint-allow: <rule> — <why>`".into(),
            ),
            Some(Allow::Valid(_)) if !allow_used => report.push(
                Rule::LintAllow,
                Severity::Warning,
                &rel_path,
                line_no,
                "lint-allow suppresses nothing on this line".into(),
            ),
            _ => {}
        }
    }
    Ok(())
}
