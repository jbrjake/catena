//! `cargo xtask <task>`: the entry point. The tasks live in the library half of this crate.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::SystemTime;
use std::{env, fs};

use anyhow::{Context, Result, bail};
use xtask::lint::{self, Severity};
use xtask::perf::{self, BUDGETS, Baseline, Measurement};

fn main() -> Result<ExitCode> {
    match env::args().nth(1).as_deref() {
        Some("lint") => run_lint(&workspace_root()?),
        Some("perf") => run_perf(&workspace_root()?),
        _ => {
            eprintln!("usage: cargo xtask <lint|perf>");
            Ok(ExitCode::from(2))
        }
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

/// Runs `catena`'s bench target and holds it to the budgets and this machine's baseline
/// (`xtask::perf`). `UPDATE_BASELINE=1` reseeds this machine's entry.
fn run_perf(root: &Path) -> Result<ExitCode> {
    let update = env::var_os("UPDATE_BASELINE").is_some_and(|v| v == "1");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let started = SystemTime::now();
    let output = Command::new(&cargo)
        .args(["bench", "-p", "catena", "--bench", "catena"])
        .current_dir(root)
        .output()
        .context("run the bench target")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{stdout}");
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        bail!("the bench target failed");
    }
    let measured = Measurement {
        medians_ns: medians(&target_dir(root, &cargo)?.join("criterion"), started)?,
        peak_rss_kib: perf::peak_rss(&stdout),
    };
    let fingerprint = this_machine()?;
    let path = root.join("bench-baseline.json");
    let mut baseline = if path.exists() {
        Baseline::from_json(&fs::read_to_string(&path)?)?
    } else {
        Baseline::default()
    };
    let previous = baseline.entry(&fingerprint).cloned();
    let report = perf::judge(&mut baseline, &fingerprint, &measured, update);

    println!("\nperf: {fingerprint:?}");
    for (id, &ns) in &measured.medians_ns {
        let budget = BUDGETS.iter().find(|(b, _)| b == id).map(|&(_, ns)| ns);
        let was = previous
            .as_ref()
            .and_then(|e| e.medians_ns.get(id).copied());
        println!(
            "  {id:<24} {:>10}  budget {:>10}  baseline {:>10}",
            perf::human(ns),
            budget.map_or_else(|| "-".to_string(), perf::human),
            was.map_or_else(|| "-".to_string(), |w| format!("{:.2}×", ns / w)),
        );
    }
    println!(
        "  peak RSS {}",
        measured
            .peak_rss_kib
            .map_or_else(|| "not reported".to_string(), |k| format!("{k} KiB"))
    );
    if report.seeded || !report.added.is_empty() {
        fs::write(&path, baseline.to_json()).context("write bench-baseline.json")?;
        println!(
            "perf: SEEDED this machine's entry in bench-baseline.json{}; commit it",
            if update { " (UPDATE_BASELINE=1)" } else { "" }
        );
    }
    for failure in &report.failures {
        println!("perf: FAIL {failure}");
    }
    Ok(if report.failures.is_empty() {
        println!("perf: OK");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Where cargo puts build output, as `cargo metadata` says (`CARGO_TARGET_DIR` may move it).
fn target_dir(root: &Path, cargo: &std::ffi::OsStr) -> Result<PathBuf> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .context("run cargo metadata")?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("cargo metadata is not JSON")?;
    metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .context("cargo metadata names no target directory")
}

/// Every bench median criterion wrote under `dir` since `since`, by id (`group/name`): an
/// estimate from an earlier run is stale and not read.
fn medians(dir: &Path, since: SystemTime) -> Result<std::collections::BTreeMap<String, f64>> {
    let mut found = std::collections::BTreeMap::new();
    for group in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let group = group?.path();
        let Ok(benches) = fs::read_dir(&group) else {
            continue;
        };
        for bench in benches {
            let estimates = bench?.path().join("new").join("estimates.json");
            let fresh = fs::metadata(&estimates)
                .and_then(|m| m.modified())
                .is_ok_and(|t| t >= since);
            if !fresh {
                continue;
            }
            let ns = perf::median_ns(&fs::read_to_string(&estimates)?)
                .with_context(|| format!("no median in {}", estimates.display()))?;
            let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
            if let (Some(g), Some(b)) = (
                name(&group),
                estimates.parent().and_then(Path::parent).and_then(name),
            ) {
                found.insert(format!("{g}/{b}"), ns);
            }
        }
    }
    Ok(found)
}

/// This machine's fingerprint: host, CPU model, cores and memory, from `/proc` on Linux and
/// `sysctl` on macOS.
fn this_machine() -> Result<perf::Fingerprint> {
    let read = |path: &str| fs::read_to_string(path).ok();
    let command = |program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let host = read("/etc/hostname")
        .or_else(|| command("hostname", &[]))
        .unwrap_or_default();
    let cpu = read("/proc/cpuinfo")
        .and_then(|info| {
            info.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, model)| model.trim().to_string())
        })
        .or_else(|| command("sysctl", &["-n", "machdep.cpu.brand_string"]))
        .unwrap_or_else(|| "unknown".to_string());
    let ram_bytes = read("/proc/meminfo")
        .and_then(|info| {
            info.lines()
                .find(|l| l.starts_with("MemTotal:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kib| kib.parse::<u64>().ok())
                .map(|kib| kib * 1024)
        })
        .or_else(|| command("sysctl", &["-n", "hw.memsize"]).and_then(|b| b.parse().ok()))
        .unwrap_or(0);
    perf::fingerprint(&host, &cpu, ram_bytes)
}
