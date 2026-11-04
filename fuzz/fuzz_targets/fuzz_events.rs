//! `fuzz_events` (plan §16.7, target 1): arbitrary input against a fixture graph, including
//! mid-drag resizes, scroll storms and clicks at `u16::MAX`. No panic, and invariants A, B and
//! C hold on the last frame. (Invariant I, the snap round trip, is not observable through the
//! public API; the core's own property test holds it.)
#![no_main]

use catena::GraphView;
use catena_fuzz::{Event, apply, check_frame};
use catena_testkit::fixtures::generated;
use libfuzzer_sys::fuzz_target;

thread_local! {
    /// The fixture laid out once; each input starts from a clone of it.
    static BASE: GraphView<String> = generated(5, 40)
        .view(GraphView::builder())
        .expect("fixtures are consistent");
}

fuzz_target!(|events: Vec<Event>| {
    let mut gv = BASE.with(GraphView::clone);
    let mut size = (80, 24);
    for event in events.iter().take(512) {
        apply(&mut gv, event, &mut size);
    }
    check_frame(&mut gv, size);
});
