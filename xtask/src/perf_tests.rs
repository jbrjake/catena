use std::collections::BTreeMap;

use super::*;

fn machine() -> Fingerprint {
    Fingerprint {
        host: "vm".into(),
        cpu: "Xeon".into(),
        cores: 4,
        ram_gib: 16,
    }
}

fn measured(scale: f64, rss: Option<u64>) -> Measurement {
    let medians_ns = BUDGETS
        .iter()
        .map(|&(id, budget)| (id.to_string(), budget / 10.0 * scale))
        .collect();
    Measurement {
        medians_ns,
        peak_rss_kib: rss,
    }
}

#[test]
fn a_new_machine_seeds_its_entry_loudly_and_passes() {
    let mut baseline = Baseline::default();
    let report = judge(
        &mut baseline,
        &machine(),
        &measured(1.0, Some(15_000)),
        false,
    );
    assert!(report.seeded, "a new machine seeds");
    assert_eq!(report.failures, Vec::<String>::new());
    assert_eq!(baseline.machines.len(), 1);
    assert_eq!(baseline.machines[0].fingerprint, machine());
    assert_eq!(baseline.machines[0].peak_rss_kib, Some(15_000));
}

#[test]
fn within_the_ratchet_passes_and_leaves_the_baseline_alone() {
    let mut baseline = Baseline::default();
    judge(
        &mut baseline,
        &machine(),
        &measured(1.0, Some(15_000)),
        false,
    );
    let before = baseline.clone();
    let report = judge(
        &mut baseline,
        &machine(),
        &measured(1.49, Some(22_000)),
        false,
    );
    assert!(!report.seeded);
    assert_eq!(report.failures, Vec::<String>::new());
    assert_eq!(baseline, before);
}

#[test]
fn past_one_and_a_half_times_fails_each_regressed_bench_and_the_rss() {
    let mut baseline = Baseline::default();
    judge(
        &mut baseline,
        &machine(),
        &measured(1.0, Some(10_000)),
        false,
    );
    let report = judge(
        &mut baseline,
        &machine(),
        &measured(1.6, Some(16_000)),
        false,
    );
    assert_eq!(
        report.failures.len(),
        BUDGETS.len() + 1,
        "{:?}",
        report.failures
    );
    assert!(
        report.failures.iter().all(|f| f.contains("owner")),
        "{:?}",
        report.failures
    );
}

#[test]
fn over_budget_fails_even_on_a_new_machine() {
    let mut baseline = Baseline::default();
    let mut slow = measured(1.0, None);
    let (id, budget) = BUDGETS[0];
    slow.medians_ns.insert(id.to_string(), budget * 1.01);
    let report = judge(&mut baseline, &machine(), &slow, false);
    assert!(report.seeded);
    assert_eq!(report.failures.len(), 1, "{:?}", report.failures);
    assert!(report.failures[0].contains(id) && report.failures[0].contains("budget"));
}

#[test]
fn update_reseeds_this_machine_only() {
    let other = Fingerprint {
        host: "laptop".into(),
        ..machine()
    };
    let mut baseline = Baseline::default();
    judge(&mut baseline, &other, &measured(1.0, Some(1)), false);
    judge(&mut baseline, &machine(), &measured(1.0, Some(1)), false);
    let report = judge(&mut baseline, &machine(), &measured(3.0, Some(9)), true);
    assert_eq!(
        report.failures,
        Vec::<String>::new(),
        "a reseed accepts the regression"
    );
    assert!(report.seeded);
    let mine = baseline.entry(&machine()).expect("mine");
    assert_eq!(mine.peak_rss_kib, Some(9));
    let theirs = baseline.entry(&other).expect("theirs");
    assert_eq!(theirs.peak_rss_kib, Some(1));
}

#[test]
fn a_bench_new_to_an_entry_is_added_without_failing() {
    let mut baseline = Baseline::default();
    let mut fewer = measured(1.0, None);
    let (id, _) = BUDGETS[1];
    fewer.medians_ns.remove(id);
    judge(&mut baseline, &machine(), &fewer, false);
    let report = judge(&mut baseline, &machine(), &measured(1.0, None), false);
    assert_eq!(report.failures, Vec::<String>::new());
    assert!(report.added.contains(&id.to_string()), "{:?}", report.added);
    assert!(
        baseline
            .entry(&machine())
            .expect("mine")
            .medians_ns
            .contains_key(id)
    );
}

#[test]
fn a_measurement_missing_a_budgeted_bench_fails() {
    let mut baseline = Baseline::default();
    let mut partial = measured(1.0, None);
    let (id, _) = BUDGETS[2];
    partial.medians_ns.remove(id);
    let report = judge(&mut baseline, &machine(), &partial, false);
    assert!(
        report
            .failures
            .iter()
            .any(|f| f.contains(id) && f.contains("not measured"))
    );
}

#[test]
fn the_baseline_round_trips_through_its_json() {
    let mut baseline = Baseline::default();
    judge(
        &mut baseline,
        &machine(),
        &measured(1.0, Some(12_345)),
        false,
    );
    let text = baseline.to_json();
    assert_eq!(Baseline::from_json(&text).expect("parses"), baseline);
    assert!(text.ends_with('\n'));
    // Stable order: the same baseline always writes the same bytes.
    assert_eq!(Baseline::from_json(&text).expect("parses").to_json(), text);
}

#[test]
fn a_malformed_baseline_is_an_error_not_an_empty_one() {
    for bad in [
        "",
        "{",
        "[]",
        r#"{"machines": 3}"#,
        r#"{"machines": [{"fingerprint": 1}]}"#,
    ] {
        assert!(Baseline::from_json(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn criterion_medians_and_the_rss_line_parse() {
    let estimates = r#"{"mean":{"point_estimate":12.0},"median":{"point_estimate":10433000.5,"standard_error":1.0}}"#;
    assert_eq!(median_ns(estimates), Some(10_433_000.5));
    assert_eq!(median_ns(r#"{"mean":{"point_estimate":1.0}}"#), None);
    assert_eq!(median_ns("not json"), None);
    let output = "layout/cold_200 time: [...]\ncatena-peak-rss-kib 14700\n";
    assert_eq!(peak_rss(output), Some(14_700));
    assert_eq!(peak_rss("catena-peak-rss-kib unknown\n"), None);
    assert_eq!(peak_rss("nothing here"), None);
}

#[test]
fn every_budget_names_a_bench_the_target_defines_and_none_twice() {
    let ids: BTreeMap<&str, usize> = BUDGETS.iter().fold(BTreeMap::new(), |mut m, &(id, _)| {
        *m.entry(id).or_default() += 1;
        m
    });
    assert!(ids.values().all(|&n| n == 1), "{ids:?}");
    let bench = include_str!("../../catena/benches/catena/main.rs");
    for &(id, budget) in &BUDGETS {
        let (group, name) = id.split_once('/').expect("group/name");
        let stem = name.split('_').next().expect("a name");
        assert!(
            bench.contains(group) && bench.contains(stem),
            "{id} is not in the bench target"
        );
        assert!(budget > 0.0);
    }
}
