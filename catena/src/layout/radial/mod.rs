//! Radial layout (plan §9.3): a ring of nodes with the optional two-level chord/Holten
//! bundling overlay. The engine itself lands at M5.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the radial engine (M5) is chord's first caller")
)]
mod chord;
