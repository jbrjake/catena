use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

use super::super::testing::{any_shape, awkward_label, every_level_and_cap, node};
use super::tests::row;
use super::*;
use crate::raster::text::char_width;

fn config() -> Config {
    Config {
        cases: 256,
        rng_seed: RngSeed::Fixed(0x6d65_7472),
        failure_persistence: None,
        ..Config::default()
    }
}

/// `text` with every character that is not whitespace swapped for another of its width.
fn same_widths(text: &str, pick: usize) -> String {
    const POOLS: [&[char]; 3] = [
        &['\u{302}', '\u{200d}', '\u{0}'],
        &['z', 'é', 'Q'],
        &['語', '😺'],
    ];
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_whitespace() {
                c
            } else {
                let pool = POOLS[char_width(c)];
                pool[(i + pick) % pool.len()]
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn labels_that_lay_out_alike_measure_alike(
        text in awkward_label(),
        pick in 0usize..6,
        shape in any_shape(),
        pin in any::<bool>(),
    ) {
        let other = same_widths(&text, pick);
        prop_assert!(crate::graph::same_layout(&text, &other));
        for (level, table) in every_level_and_cap() {
            let a = measure(&node(&text, shape, pin), level, &table);
            let b = measure(&node(&other, shape, pin), level, &table);
            prop_assert_eq!(
                (a.width(), a.height()),
                (b.width(), b.height()),
                "{:?} at {:?}: {:?} vs {:?}", table, level, a, b
            );
        }
    }

    #[test]
    fn every_form_keeps_to_its_cap_and_measures_what_it_draws(
        text in awkward_label(),
        shape in any_shape(),
        pin in any::<bool>(),
    ) {
        let spec = node(&text, shape, pin);
        for (level, table) in every_level_and_cap() {
            let form = measure(&spec, level, &table);
            if let Some(cap) = table.cap(level) {
                prop_assert!(form.width() <= cap, "{:?} at {:?}: {:?}", table, level, form);
            }
            prop_assert!(form.width() >= 1 && form.height() >= 1);
            if let FormShape::Boxed { lines, .. } = form.shape() {
                prop_assert!(form.width() >= 3 && form.height() >= 3);
                let inner = usize::from(form.width() - 2);
                prop_assert!(lines.len() + 2 <= usize::from(form.height()));
                for line in lines {
                    prop_assert!(display_width(line) <= inner);
                }
            } else {
                prop_assert_eq!(form.height(), 1);
                prop_assert_eq!(usize::from(form.width()), display_width(&row(&form)));
            }
        }
    }
}
