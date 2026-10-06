//! Rules about the repository's shape rather than its source lines: the plan §3.1 build
//! scaffold, and the post-harvest check.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::{Config, Report, Rule, Severity, is_seed_staged, read, rel, walk};

static MEMBERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\bmembers\s*=\s*\[(.*?)\]").expect("valid regex"));
static QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]+)""#).expect("valid regex"));

/// Runs every scaffold rule.
pub(crate) fn check(root: &Path, config: &Config, report: &mut Report) -> Result<()> {
    rustflags_in_automation(root, report)?;
    rustflags_in_cargo_config(root, report)?;
    one_target_per_crate(root, report)?;
    harvest(root, config, report)?;
    Ok(())
}

/// `RUSTFLAGS` anywhere in a script, hook or workflow forks `target/` (plan §3.1).
fn rustflags_in_automation(root: &Path, report: &mut Report) -> Result<()> {
    for dir in ["scripts", ".githooks", ".github"] {
        for path in walk(&root.join(dir))? {
            let text = read(&path)?;
            for (idx, line) in text.lines().enumerate() {
                if line.contains("RUSTFLAGS") {
                    report.push(
                        Rule::Rustflags,
                        Severity::Error,
                        &rel(root, &path),
                        idx + 1,
                        "RUSTFLAGS forks target/: warnings fail the gate's own commands \
                         (plan §3.1)"
                            .into(),
                    );
                }
            }
        }
    }
    Ok(())
}

/// Any `rustflags` key in the cargo config forks `target/` the same way (plan §3.1).
fn rustflags_in_cargo_config(root: &Path, report: &mut Report) -> Result<()> {
    for name in ["config.toml", "config"] {
        let path = root.join(".cargo").join(name);
        if !path.is_file() {
            continue;
        }
        let text = read(&path)?;
        for (idx, line) in text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or("");
            if code.to_ascii_lowercase().contains("rustflags") {
                report.push(
                    Rule::Rustflags,
                    Severity::Error,
                    &rel(root, &path),
                    idx + 1,
                    "rustflags in the cargo config forks target/: the config holds only the \
                     xtask alias (plan §3.1)"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

/// Every workspace member has at most one test and one bench target, and switches off cargo's
/// auto-discovery so a stray file cannot become a new target (plan §3.1).
fn one_target_per_crate(root: &Path, report: &mut Report) -> Result<()> {
    let manifest = read(&root.join("Cargo.toml"))?;
    let members: Vec<String> = MEMBERS
        .captures(&manifest)
        .and_then(|caps| caps.get(1))
        .map(|list| {
            QUOTED
                .captures_iter(list.as_str())
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .collect()
        })
        .unwrap_or_default();
    for member in members {
        let dir = root.join(&member);
        let manifest_path = dir.join("Cargo.toml");
        let text = read(&manifest_path)?;
        let manifest_rel = rel(root, &manifest_path);
        for (switch, table, kind) in [
            ("autotests", "[[test]]", "tests"),
            ("autobenches", "[[bench]]", "benches"),
        ] {
            let mut problems = Vec::new();
            if !text.lines().any(|l| is_false_switch(l, switch)) {
                problems.push(format!("{switch} = false is missing"));
            }
            let declared = text.lines().filter(|l| l.trim() == table).count();
            if declared > 1 {
                problems.push(format!("{declared} {table} targets declared"));
            }
            let on_disk = target_candidates(&dir.join(kind))?;
            if on_disk > 1 {
                problems.push(format!("{on_disk} targets under {member}/{kind}/"));
            }
            for problem in problems {
                report.push(
                    Rule::OneTarget,
                    Severity::Error,
                    &manifest_rel,
                    0,
                    format!("{problem}: one {kind} target per crate (plan §3.1)"),
                );
            }
        }
    }
    Ok(())
}

fn is_false_switch(line: &str, switch: &str) -> bool {
    let code = line.split('#').next().unwrap_or("");
    code.split_once('=')
        .is_some_and(|(key, value)| key.trim() == switch && value.trim() == "false")
}

/// What cargo would compile as separate targets in `dir`: each direct `*.rs` child and each
/// subdirectory holding a `main.rs`.
fn target_candidates(dir: &Path) -> Result<usize> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let is_rs_file = path.is_file() && path.extension().is_some_and(|e| e == "rs");
        let is_main_dir = path.is_dir() && path.join("main.rs").is_file();
        if is_rs_file || is_main_dir {
            count += 1;
        }
    }
    Ok(count)
}

/// Counts staged `*.seed.rs` files; once the harvest is complete, any of them, or a `seed/`
/// directory, is an error (plan §17, §18).
fn harvest(root: &Path, config: &Config, report: &mut Report) -> Result<()> {
    for path in walk(root)? {
        if is_seed_staged(&path) {
            report.staged_seed_files += 1;
            if config.harvest_complete {
                report.push(
                    Rule::HarvestComplete,
                    Severity::Error,
                    &rel(root, &path),
                    0,
                    "staged seed file after the harvest is complete (plan §18)".into(),
                );
            }
        }
    }
    if config.harvest_complete && root.join("seed").exists() {
        report.push(
            Rule::HarvestComplete,
            Severity::Error,
            "seed",
            0,
            "seed/ still exists after the harvest is complete (plan §18)".into(),
        );
    }
    Ok(())
}
