//! Each rule is proven live by planting a violation in a scratch repository and watching the
//! rule fire at the planted line; the near-misses beside each plant prove it fires only there.

mod masking;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Config, Finding, Report, Rule, Severity, run};

/// A scratch repository with a clean minimal workspace, deleted on drop.
struct Tree {
    root: PathBuf,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

const MEMBER_MANIFEST: &str = "[package]\nname = \"m\"\nautotests = false\nautobenches = false\n";

impl Tree {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "xtask-lint-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("clear a stale scratch tree");
        }
        let tree = Tree { root };
        tree.write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"catena\", \"catena-ratatui\", \"catena-testkit\"]\n",
        );
        for krate in ["catena", "catena-ratatui", "catena-testkit"] {
            tree.write(&format!("{krate}/Cargo.toml"), MEMBER_MANIFEST);
            tree.write(&format!("{krate}/src/lib.rs"), "//! A crate.\n");
            tree.write(&format!("{krate}/tests/{krate}/main.rs"), "//! Tests.\n");
        }
        tree
    }

    fn write(&self, rel: &str, contents: &str) -> &Self {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().expect("a relative path has a parent"))
            .expect("create parent directories");
        fs::write(&path, contents).expect("write a planted file");
        self
    }

    fn lint_with(&self, config: &Config) -> Report {
        run(&self.root, config).expect("lint runs over the scratch tree")
    }

    fn lint(&self) -> Report {
        self.lint_with(&Config::default())
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// `(path, line)` of every finding of `rule` at `severity`.
fn hits(report: &Report, rule: Rule, severity: Severity) -> Vec<(String, usize)> {
    report
        .findings
        .iter()
        .filter(|f| f.rule == rule && f.severity == severity)
        .map(|f: &Finding| (f.path.clone(), f.line))
        .collect()
}

fn errors(report: &Report, rule: Rule) -> Vec<(String, usize)> {
    hits(report, rule, Severity::Error)
}

fn at(path: &str, line: usize) -> (String, usize) {
    (path.to_string(), line)
}

#[test]
fn the_scratch_tree_starts_clean() {
    let tree = Tree::new();
    let report = tree.lint();
    assert_eq!(report.findings.len(), 0, "{:?}", report.findings);
    assert_eq!(report.files_scanned, 3);
}

#[test]
fn time_rule_catches_clock_types_but_not_prose() {
    let tree = Tree::new();
    tree.write(
        "catena/src/a.rs",
        "// Instant in a comment is fine.\nlet t = std::time::Instant::now();\n\
         let s = \"SystemTime\";\nlet u = SystemTime::now();\nlet d = Duration::ZERO;\n",
    );
    tree.write("catena-testkit/src/b.rs", "\n\nuse std::time::Instant;\n");
    let report = tree.lint();
    let found = errors(&report, Rule::Time);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena-testkit/src/b.rs", 3),
            at("catena/src/a.rs", 2),
            at("catena/src/a.rs", 4)
        ]
    );
}

#[test]
fn rng_rule_catches_entropy_sources() {
    let tree = Tree::new();
    tree.write(
        "catena/src/a.rs",
        "let r = rand::random::<u8>();\nlet g = thread_rng();\nlet operand = 1;\n\
         let s = Rng::from_entropy();\n",
    );
    let found = errors(&tree.lint(), Rule::Rng);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena/src/a.rs", 1),
            at("catena/src/a.rs", 2),
            at("catena/src/a.rs", 4)
        ]
    );
}

#[test]
fn fmath_rule_bans_std_transcendentals_outside_fmath() {
    let tree = Tree::new();
    let body = "let a = x.sin();\nlet b = f64::atan2(y, x);\nlet c = x.sqrt();\n\
                let d = x.sin_cos();\nlet e = x.round();\n";
    tree.write("catena/src/a.rs", body);
    tree.write("catena/src/fmath.rs", body);
    tree.write("catena-testkit/src/b.rs", body);
    let found = errors(&tree.lint(), Rule::Fmath);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena/src/a.rs", 1),
            at("catena/src/a.rs", 2),
            at("catena/src/a.rs", 4)
        ]
    );
}

#[test]
fn float_order_rule_bans_partial_cmp_unwrap_and_expect() {
    let tree = Tree::new();
    tree.write(
        "catena-ratatui/src/a.rs",
        "v.sort_by(|a, b| a.partial_cmp(b).unwrap());\nv.sort_by(|a, b| a.total_cmp(b));\n\
         let o = a.partial_cmp(&b).expect(\"finite\");\n",
    );
    let found = errors(&tree.lint(), Rule::FloatOrder);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena-ratatui/src/a.rs", 1),
            at("catena-ratatui/src/a.rs", 3)
        ]
    );
}

#[test]
fn display_width_rule_flags_byte_length_of_display_text_only() {
    let tree = Tree::new();
    tree.write(
        "catena/src/a.rs",
        "let w = label.len();\nlet n = labels.len();\nlet f = filename.len();\n\
         let t = self.node_title.len();\nlet s = node.symbol . len ();\nlet v = values.len();\n",
    );
    let found = errors(&tree.lint(), Rule::DisplayWidth);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena/src/a.rs", 1),
            at("catena/src/a.rs", 4),
            at("catena/src/a.rs", 5)
        ]
    );
}

#[test]
fn unwrap_rule_exempts_test_code_only() {
    let tree = Tree::new();
    tree.write(
        "catena/src/a.rs",
        "fn f() { g().unwrap(); }\n\
         #[cfg(test)]\nmod tests {\n    fn t() { g().unwrap(); }\n    fn u(x: [u8; 2]) {}\n}\n\
         fn h() { g().unwrap_or(0); g().unwrap(); }\n\
         #[test]\nfn k() {\n    g().unwrap();\n}\n\
         #[cfg(test)]\n#[path = \"a_tests.rs\"]\nmod more;\nfn m() { g().unwrap(); }\n",
    );
    tree.write("catena/src/a_tests.rs", "fn t() { g().unwrap(); }\n");
    tree.write("catena/src/tests.rs", "fn t() { g().unwrap(); }\n");
    let found = errors(&tree.lint(), Rule::Unwrap);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena/src/a.rs", 1),
            at("catena/src/a.rs", 7),
            at("catena/src/a.rs", 15)
        ]
    );
}

#[test]
fn io_fence_rule_covers_core_and_widget_but_not_bins_or_testkit() {
    let tree = Tree::new();
    let body = "println!(\"x\");\nlet t = std::fs::read(p);\nlet v = std::env::var(k);\n\
                dbg!(x);\nlet w = std::iox;\n";
    tree.write("catena/src/a.rs", body);
    tree.write("catena-ratatui/src/b.rs", body);
    tree.write("catena-ratatui/src/bin/catena-demo.rs", body);
    tree.write("catena-testkit/src/c.rs", body);
    let found = errors(&tree.lint(), Rule::IoFence);
    assert_eq!(found.len(), 8, "{found:?}");
    let mut expected = Vec::new();
    for file in ["catena-ratatui/src/b.rs", "catena/src/a.rs"] {
        expected.extend((1..=4).map(|line| at(file, line)));
    }
    assert_eq!(found, expected);
}

#[test]
fn file_length_rule_warns_over_500_and_fails_over_750() {
    let tree = Tree::new();
    let lines = |n: usize| "x\n".repeat(n);
    tree.write("catena/src/ok.rs", &lines(500));
    tree.write("catena/src/long.rs", &lines(501));
    tree.write("catena-testkit/tests/catena-testkit/huge.rs", &lines(751));
    tree.write("xtask/src/edge.rs", &lines(750));
    let report = tree.lint();
    let warned = hits(&report, Rule::FileLength, Severity::Warning);
    let failed = errors(&report, Rule::FileLength);
    assert_eq!(
        (warned.len(), failed.len()),
        (2, 1),
        "{warned:?} {failed:?}"
    );
    assert_eq!(
        warned,
        [at("catena/src/long.rs", 0), at("xtask/src/edge.rs", 0)]
    );
    assert_eq!(
        failed,
        [at("catena-testkit/tests/catena-testkit/huge.rs", 0)]
    );
}

#[test]
fn lint_allow_silences_the_named_rule_with_a_reason() {
    let tree = Tree::new();
    tree.write(
        "catena/src/a.rs",
        "g().unwrap(); // lint-allow: unwrap — proven non-empty above\n\
         g().unwrap(); // lint-allow: unwrap\n\
         g().unwrap(); // lint-allow: time — wrong rule\n\
         let x = 1; // lint-allow: nonsense — no such rule\n\
         let y = 2; // lint-allow: rng -- nothing to silence\n",
    );
    let report = tree.lint();
    let unwraps = errors(&report, Rule::Unwrap);
    assert_eq!(unwraps.len(), 2, "{unwraps:?}");
    assert_eq!(
        unwraps,
        [at("catena/src/a.rs", 2), at("catena/src/a.rs", 3)]
    );
    let malformed = errors(&report, Rule::LintAllow);
    assert_eq!(malformed.len(), 2, "{malformed:?}");
    assert_eq!(
        malformed,
        [at("catena/src/a.rs", 2), at("catena/src/a.rs", 4)]
    );
    let unused = hits(&report, Rule::LintAllow, Severity::Warning);
    assert_eq!(unused.len(), 2, "{unused:?}");
    assert_eq!(unused, [at("catena/src/a.rs", 3), at("catena/src/a.rs", 5)]);
    assert_eq!(report.allowances.len(), 1);
    assert_eq!(report.allowances.get("unwrap"), Some(&1));
}

#[test]
fn rustflags_rule_catches_scripts_hooks_and_workflows() {
    let tree = Tree::new();
    tree.write(
        "scripts/smoke.sh",
        "#!/bin/sh\nexport RUSTFLAGS=\"-D warnings\"\n",
    );
    tree.write(".githooks/pre-push", "RUSTFLAGS=x cargo test\n");
    tree.write(
        ".github/workflows/ci.yml",
        "env:\n  RUSTDOCFLAGS: -D warnings\n  RUSTFLAGS: -D warnings\n",
    );
    let found = errors(&tree.lint(), Rule::Rustflags);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        found,
        [
            at(".githooks/pre-push", 1),
            at(".github/workflows/ci.yml", 3),
            at("scripts/smoke.sh", 2)
        ]
    );
}

#[test]
fn rustflags_rule_catches_the_cargo_config_key_but_not_comments() {
    let tree = Tree::new();
    tree.write(
        ".cargo/config.toml",
        "# no rustflags here, ever\n[alias]\nxtask = \"run --package xtask --\"\n\
         [build]\nrustflags = [\"-Dwarnings\"]\n",
    );
    let found = errors(&tree.lint(), Rule::Rustflags);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found, [at(".cargo/config.toml", 5)]);
}

#[test]
fn one_target_rule_catches_extra_targets_and_missing_switches() {
    let tree = Tree::new();
    tree.write("catena/tests/extra.rs", "//! A second target.\n");
    tree.write("catena-ratatui/benches/a/main.rs", "fn main() {}\n");
    tree.write("catena-ratatui/benches/b.rs", "fn main() {}\n");
    tree.write(
        "catena-testkit/Cargo.toml",
        "[package]\nname = \"m\"\nautotests = false # one target\n\n\
         [[test]]\nname = \"a\"\n[[test]]\nname = \"b\"\n",
    );
    let found = errors(&tree.lint(), Rule::OneTarget);
    assert_eq!(found.len(), 4, "{found:?}");
    assert_eq!(
        found,
        [
            at("catena-ratatui/Cargo.toml", 0),
            at("catena-testkit/Cargo.toml", 0),
            at("catena-testkit/Cargo.toml", 0),
            at("catena/Cargo.toml", 0),
        ]
    );
}

#[test]
fn harvest_rule_is_dormant_until_the_harvest_completes() {
    let tree = Tree::new();
    tree.write(
        "catena/src/raster/braille.seed.rs",
        "let t = Instant::now(); x.unwrap();\n",
    );
    tree.write("seed/graph/zoom.rs", "let t = Instant::now();\n");
    let before = tree.lint();
    assert_eq!(before.findings.len(), 0, "{:?}", before.findings);
    assert_eq!(before.staged_seed_files, 1);
    let after = tree.lint_with(&Config {
        harvest_complete: true,
    });
    let found = errors(&after, Rule::HarvestComplete);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(
        found,
        [at("catena/src/raster/braille.seed.rs", 0), at("seed", 0)]
    );
}

#[test]
fn the_real_repository_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root");
    let report = run(
        root,
        &Config {
            harvest_complete: super::HARVEST_COMPLETE,
        },
    )
    .expect("lint runs over the repository");
    assert_eq!(report.count(Severity::Error), 0, "{:?}", report.findings);
    assert!(report.files_scanned > 0);
}

#[test]
fn every_rule_name_round_trips() {
    for rule in Rule::ALL {
        assert_eq!(Rule::from_name(rule.name()), Some(rule));
    }
    assert_eq!(Rule::from_name("nope"), None);
}
