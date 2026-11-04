//! `cargo xtask perf`, behind `scripts/check-perf.sh`: the performance ratchet (plan §15.1).
//!
//! The bench target's medians (criterion's, of at least ten samples after warm-up) and its peak
//! resident set are held to two bars. Each budgeted bench must be within its plan §15.1 budget,
//! on any machine. And each bench and the peak RSS must be within 1.5× of this machine's entry
//! in the committed `bench-baseline.json`; a regression past that needs the owner's approval,
//! after which `UPDATE_BASELINE=1` reseeds the entry. A machine with no entry seeds one, loudly,
//! instead of failing. A machine is its host name, CPU model, core count and memory.
//!
//! This module is the judgment, free of I/O; `main.rs` runs the benches and reads the files.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

/// How far past this machine's baseline a median or the peak RSS may go (plan §15.1).
pub const RATCHET: f64 = 1.5;

/// The plan §15.1 budgets of the benches that exist, in nanoseconds of median.
pub const BUDGETS: [(&str, f64); 6] = [
    ("layout/cold_200", 50e6),
    ("layout/cold_1000", 400e6),
    ("layout/warm_5pct_200", 10e6),
    ("frame/rebuilt_200", 3e6),
    ("frame/rebuilt_1000", 16e6),
    ("update/property_1000", 100e3),
];

/// What tells one machine's baseline from another's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub host: String,
    pub cpu: String,
    pub cores: u32,
    pub ram_gib: u64,
}

/// One run of the bench target.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// Each bench's median, by criterion id (`group/name`).
    pub medians_ns: BTreeMap<String, f64>,
    /// The bench process's peak resident set, where the platform reports it.
    pub peak_rss_kib: Option<u64>,
}

/// One machine's entry in `bench-baseline.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub fingerprint: Fingerprint,
    pub medians_ns: BTreeMap<String, f64>,
    pub peak_rss_kib: Option<u64>,
}

/// `bench-baseline.json`: an entry per machine, in the order they were first seeded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Baseline {
    pub machines: Vec<Entry>,
}

/// What [`judge`] found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// Every bar a bench or the RSS failed, one line each.
    pub failures: Vec<String>,
    /// Whether this machine's entry was written afresh: a new machine, or an update.
    pub seeded: bool,
    /// Benches new to this machine's entry, added to it.
    pub added: Vec<String>,
}

impl Baseline {
    /// This machine's entry, if it has one.
    #[must_use]
    pub fn entry(&self, fingerprint: &Fingerprint) -> Option<&Entry> {
        self.machines.iter().find(|e| &e.fingerprint == fingerprint)
    }

    /// Parses `bench-baseline.json`.
    ///
    /// # Errors
    ///
    /// When `text` is not the shape [`Baseline::to_json`] writes; a broken baseline is never
    /// read as an empty one, which would reseed every machine silently.
    pub fn from_json(text: &str) -> Result<Self> {
        let root: Value = serde_json::from_str(text).context("bench-baseline.json is not JSON")?;
        let machines = root
            .get("machines")
            .and_then(Value::as_array)
            .context("bench-baseline.json has no `machines` array")?;
        let mut baseline = Baseline::default();
        for machine in machines {
            baseline.machines.push(entry_from(machine)?);
        }
        Ok(baseline)
    }

    /// The baseline as pretty JSON with a trailing newline, keys sorted, so the same baseline
    /// always writes the same bytes.
    #[must_use]
    pub fn to_json(&self) -> String {
        let machines: Vec<Value> = self
            .machines
            .iter()
            .map(|e| {
                let medians: Map<String, Value> = e
                    .medians_ns
                    .iter()
                    .map(|(id, ns)| (id.clone(), json!(ns.round())))
                    .collect();
                json!({
                    "fingerprint": {
                        "host": e.fingerprint.host,
                        "cpu": e.fingerprint.cpu,
                        "cores": e.fingerprint.cores,
                        "ram_gib": e.fingerprint.ram_gib,
                    },
                    "medians_ns": medians,
                    "peak_rss_kib": e.peak_rss_kib,
                })
            })
            .collect();
        let mut text = serde_json::to_string_pretty(&json!({ "machines": machines }))
            .unwrap_or_else(|_| String::from("{}"));
        text.push('\n');
        text
    }
}

fn entry_from(machine: &Value) -> Result<Entry> {
    let fp = machine
        .get("fingerprint")
        .context("an entry has no fingerprint")?;
    let text = |key: &str| -> Result<String> {
        fp.get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .with_context(|| format!("a fingerprint has no `{key}`"))
    };
    let number = |key: &str| -> Result<u64> {
        fp.get(key)
            .and_then(Value::as_u64)
            .with_context(|| format!("a fingerprint has no `{key}`"))
    };
    let fingerprint = Fingerprint {
        host: text("host")?,
        cpu: text("cpu")?,
        cores: u32::try_from(number("cores")?).context("`cores` is too large")?,
        ram_gib: number("ram_gib")?,
    };
    let mut medians_ns = BTreeMap::new();
    let medians = machine
        .get("medians_ns")
        .and_then(Value::as_object)
        .context("an entry has no `medians_ns` object")?;
    for (id, ns) in medians {
        let ns = ns
            .as_f64()
            .with_context(|| format!("median of {id} is not a number"))?;
        medians_ns.insert(id.clone(), ns);
    }
    let peak_rss_kib = match machine.get("peak_rss_kib") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().context("`peak_rss_kib` is not a count")?),
    };
    Ok(Entry {
        fingerprint,
        medians_ns,
        peak_rss_kib,
    })
}

/// Holds `measured` on `fingerprint` to the budgets and to its baseline entry, which it seeds
/// when the machine is new or `update` is set, and to which it adds benches the entry lacks.
pub fn judge(
    baseline: &mut Baseline,
    fingerprint: &Fingerprint,
    measured: &Measurement,
    update: bool,
) -> Report {
    let mut report = Report::default();
    for &(id, budget) in &BUDGETS {
        match measured.medians_ns.get(id) {
            None => report.failures.push(format!("{id}: not measured")),
            Some(&ns) if ns > budget => report.failures.push(format!(
                "{id}: median {} over its budget of {}",
                human(ns),
                human(budget)
            )),
            Some(_) => {}
        }
    }

    let fresh = Entry {
        fingerprint: fingerprint.clone(),
        medians_ns: measured.medians_ns.clone(),
        peak_rss_kib: measured.peak_rss_kib,
    };
    let Some(index) = baseline
        .machines
        .iter()
        .position(|e| &e.fingerprint == fingerprint)
    else {
        baseline.machines.push(fresh);
        report.seeded = true;
        return report;
    };
    if update {
        baseline.machines[index] = fresh;
        report.seeded = true;
        return report;
    }

    let entry = &mut baseline.machines[index];
    for (id, &ns) in &measured.medians_ns {
        match entry.medians_ns.get(id) {
            Some(&was) if ns > was * RATCHET => report.failures.push(format!(
                "{id}: median {} is {:.2}× this machine's baseline {}; past {RATCHET}× needs the \
                 owner's approval, then UPDATE_BASELINE=1",
                human(ns),
                ns / was,
                human(was)
            )),
            Some(_) => {}
            None => {
                entry.medians_ns.insert(id.clone(), ns);
                report.added.push(id.clone());
            }
        }
    }
    if let (Some(now), Some(was)) = (measured.peak_rss_kib, entry.peak_rss_kib) {
        // RATCHET is 3/2, compared exactly in integers.
        if now.saturating_mul(2) > was.saturating_mul(3) {
            report.failures.push(format!(
                "peak RSS {now} KiB is past {RATCHET}× this machine's baseline {was} KiB; that \
                 needs the owner's approval, then UPDATE_BASELINE=1"
            ));
        }
    } else if entry.peak_rss_kib.is_none() {
        entry.peak_rss_kib = measured.peak_rss_kib;
    }
    report
}

/// `ns` as the largest unit that keeps it at least 1.
#[must_use]
pub fn human(ns: f64) -> String {
    if ns >= 1e6 {
        format!("{:.2} ms", ns / 1e6)
    } else if ns >= 1e3 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{ns:.0} ns")
    }
}

/// The median point estimate, in nanoseconds, of a criterion `estimates.json`.
#[must_use]
pub fn median_ns(estimates: &str) -> Option<f64> {
    let value: Value = serde_json::from_str(estimates).ok()?;
    value.get("median")?.get("point_estimate")?.as_f64()
}

/// The peak resident set the bench target printed, `catena-peak-rss-kib <n>`.
#[must_use]
pub fn peak_rss(output: &str) -> Option<u64> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("catena-peak-rss-kib "))
        .and_then(|n| n.trim().parse().ok())
}

/// The fingerprint fields read from the platform: the host, the CPU's model and the memory in
/// bytes. Cores come from the standard library.
///
/// # Errors
///
/// When the core count is unknown.
pub fn fingerprint(host: &str, cpu: &str, ram_bytes: u64) -> Result<Fingerprint> {
    let cores = std::thread::available_parallelism()
        .context("the core count is unknown")?
        .get();
    if host.trim().is_empty() {
        bail!("the host name is empty");
    }
    Ok(Fingerprint {
        host: host.trim().to_string(),
        cpu: cpu.trim().to_string(),
        cores: u32::try_from(cores).unwrap_or(u32::MAX),
        ram_gib: (ram_bytes + (1 << 29)) >> 30,
    })
}

#[cfg(test)]
#[path = "perf_tests.rs"]
mod tests;
