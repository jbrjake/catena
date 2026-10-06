//! `cargo xtask <task>`: the entry point. The tasks live in the library half of this crate.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use xtask::lint::{self, Severity};

fn main() -> Result<ExitCode> {
    if std::env::args().nth(1).as_deref() == Some("lint") {
        run_lint(&workspace_root()?)
    } else {
        eprintln!("usage: cargo xtask lint");
        Ok(ExitCode::from(2))
    }
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask's manifest directory has no parent")
}

fn run_lint(root: &Path) -> Result<ExitCode> {
    let config = lint::Config {
        harvest_complete: lint::HARVEST_COMPLETE,
    };
    let report = lint::run(root, &config)?;
    for finding in &report.findings {
        println!("{finding}");
    }
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warning);
    let allowances = if report.allowances.is_empty() {
        "none".to_string()
    } else {
        report
            .allowances
            .iter()
            .map(|(rule, n)| format!("{rule}×{n}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!(
        "xtask lint: {} files, {errors} errors, {warnings} warnings; lint-allow: {allowances}; \
         staged seed files: {}",
        report.files_scanned, report.staged_seed_files
    );
    Ok(if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
