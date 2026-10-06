//! Tree layout (plan §9.3): centered-parent subtree packing with orthogonal connectors. The
//! engine (`TreeSpec`, the spanning-forest guard, scene emission) lands at M4.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the tree engine (M4) is its first caller")
)]
mod tree_layout;
