//! `cargo xtask lint`: the plan §17 tripwires.
//!
//! The rules are regexes over source text, not a type-checked analysis. Source rules run on the
//! three published crates' `src/` with comments and string literals blanked ([`mask`]), so
//! prose that names a banned item passes. A false positive is silenced by a comment on the
//! offending line, `// lint-allow: <rule> — <why>`, and every allowance is counted in the
//! summary so they stay visible. `seed/` and staged `*.seed.rs` files are never scanned
//! (plan §18).

pub mod mask;
mod scaffold;
mod source;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Every rule `cargo xtask lint` enforces, by the name a `lint-allow` comment uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// `Instant`/`SystemTime`: time enters only through `tick(dt)` (plan §10.4).
    Time,
    /// `thread_rng`/`from_entropy`/`rand::`: no randomness in the library (plan §8.1).
    Rng,
    /// A `std` transcendental float method in `catena/src` outside `fmath.rs` (plan §11).
    Fmath,
    /// `partial_cmp(..).unwrap()`: float ordering goes through `total_cmp` (plan §11).
    FloatOrder,
    /// `.len()` on a label, title, name or symbol: display width is unicode-width (plan §5).
    DisplayWidth,
    /// `.unwrap()` outside test code.
    Unwrap,
    /// I/O, environment, process or printing in `catena` or `catena-ratatui` (plan §2.2).
    IoFence,
    /// A file over the 750-line ceiling (error) or the 500-line target (warning).
    FileLength,
    /// `RUSTFLAGS` in a script, hook or workflow, or `rustflags` in `.cargo/config.toml`.
    Rustflags,
    /// More than one test or bench target in a crate, or the manifest switches missing.
    OneTarget,
    /// A `*.seed.rs` file or a `seed/` directory once the harvest is complete (plan §18).
    HarvestComplete,
    /// A malformed or unused `lint-allow` comment.
    LintAllow,
}

impl Rule {
    /// Every rule, in report order.
    pub const ALL: [Rule; 12] = [
        Rule::Time,
        Rule::Rng,
        Rule::Fmath,
        Rule::FloatOrder,
        Rule::DisplayWidth,
        Rule::Unwrap,
        Rule::IoFence,
        Rule::FileLength,
        Rule::Rustflags,
        Rule::OneTarget,
        Rule::HarvestComplete,
        Rule::LintAllow,
    ];

    /// The rule's name, as written in a `lint-allow` comment.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Rule::Time => "time",
            Rule::Rng => "rng",
            Rule::Fmath => "fmath",
            Rule::FloatOrder => "float-order",
            Rule::DisplayWidth => "display-width",
            Rule::Unwrap => "unwrap",
            Rule::IoFence => "io-fence",
            Rule::FileLength => "file-length",
            Rule::Rustflags => "rustflags",
            Rule::OneTarget => "one-target",
            Rule::HarvestComplete => "harvest-complete",
            Rule::LintAllow => "lint-allow",
        }
    }

    /// The rule with this name, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Rule> {
        Rule::ALL.into_iter().find(|rule| rule.name() == name)
    }
}

/// Whether a finding fails the lint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Fails `cargo xtask lint`.
    Error,
    /// Reported, but the lint still passes.
    Warning,
}

/// One rule violation at one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: Rule,
    pub severity: Severity,
    /// Repository-relative, with `/` separators.
    pub path: String,
    /// 1-based; 0 when the finding is about the whole file.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        if self.line == 0 {
            write!(
                f,
                "{}: {level}[{}]: {}",
                self.path,
                self.rule.name(),
                self.message
            )
        } else {
            write!(
                f,
                "{}:{}: {level}[{}]: {}",
                self.path,
                self.line,
                self.rule.name(),
                self.message
            )
        }
    }
}

/// Knobs that change which rules are live.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Turns on the post-M5 rule that no `*.seed.rs` file or `seed/` directory remains. Off by
    /// default; [`HARVEST_COMPLETE`] is what `cargo xtask lint` passes.
    pub harvest_complete: bool,
}

/// Whether the seed harvest is finished. M5 flips this (plan §17, §19).
pub const HARVEST_COMPLETE: bool = false;

/// Everything one lint run found.
#[derive(Debug, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Allowances that suppressed a finding, by rule name.
    pub allowances: BTreeMap<&'static str, usize>,
    pub files_scanned: usize,
    /// `*.seed.rs` files awaiting their port commit.
    pub staged_seed_files: usize,
}

impl Report {
    /// How many findings have this severity.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    fn push(&mut self, rule: Rule, severity: Severity, path: &str, line: usize, message: String) {
        self.findings.push(Finding {
            rule,
            severity,
            path: path.to_string(),
            line,
            message,
        });
    }
}

/// The crates whose `src/` the source rules scan.
pub(crate) const CRATES: [&str; 3] = ["catena", "catena-ratatui", "catena-testkit"];

/// Runs every rule over the repository at `root`.
///
/// # Errors
///
/// Fails if a directory or file under `root` cannot be read.
pub fn run(root: &Path, config: &Config) -> Result<Report> {
    let mut report = Report::default();
    source::check(root, &mut report)?;
    scaffold::check(root, config, &mut report)?;
    report
        .findings
        .sort_by(|a, b| (&a.path, a.line, a.rule).cmp(&(&b.path, b.line, b.rule)));
    Ok(report)
}

/// Every file under `dir`, recursively, sorted, skipping `target/` and `.git/`. A missing
/// directory yields nothing.
pub(crate) fn walk(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if dir.is_dir() {
        walk_into(dir, &mut out)?;
    }
    out.sort();
    Ok(out)
}

fn walk_into(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if name != "target" && name != ".git" {
                walk_into(&path, out)?;
            }
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// `path` relative to `root`, with `/` separators on every platform.
pub(crate) fn rel(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether `path` is a staged harvest file, which no rule scans.
pub(crate) fn is_seed_staged(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".seed.rs"))
}

/// Reads a file as text.
pub(crate) fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}
