use super::*;

/// A path 0 - 1 - 2 - 3 - 4 - 5.
fn path() -> Vec<Vec<usize>> {
    (0..6)
        .map(|i: usize| {
            [i.checked_sub(1), (i < 5).then_some(i + 1)]
                .into_iter()
                .flatten()
                .collect()
        })
        .collect()
}

#[test]
fn hops_count_from_the_nearest_source_each_at_its_own_hop() {
    let adjacent = path();
    let mut sources = vec![None; 6];
    sources[0] = Some(0);
    assert_eq!(hops(&adjacent, &sources), [0, 1, 2, 3, 4, 5]);
    sources[4] = Some(1);
    assert_eq!(hops(&adjacent, &sources), [0, 1, 2, 2, 1, 2]);
    sources[1] = Some(1);
    assert_eq!(
        hops(&adjacent, &sources),
        [0, 1, 2, 2, 1, 2],
        "a hop-1 source next to a hop-0 one is one hop out either way"
    );
    assert_eq!(hops(&adjacent, &[None; 6]), [u32::MAX; 6]);
}

#[test]
fn the_tether_grows_with_hops_and_holds_beyond_the_reach() {
    let params = ForceParams {
        tether_reach: 3,
        tether_near: 0.5,
        tether_far: 4.5,
        ..ForceParams::default()
    };
    let at = |h| mobility_at(&params, h);
    assert_eq!(at(0), Mobility::Tethered(0.5));
    assert_eq!(at(1), Mobility::Tethered(0.5));
    assert_eq!(at(2), Mobility::Tethered(2.5));
    assert_eq!(at(3), Mobility::Tethered(4.5));
    assert_eq!(at(4), Mobility::Held);
    assert_eq!(at(u32::MAX), Mobility::Held, "no change reaches it");
    let one = ForceParams {
        tether_reach: 1,
        ..params
    };
    assert_eq!(mobility_at(&one, 1), Mobility::Tethered(0.5));
    assert_eq!(mobility_at(&one, 2), Mobility::Held);
}
