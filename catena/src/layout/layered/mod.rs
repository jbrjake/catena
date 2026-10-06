//! Layered (Sugiyama) layout (plan §9.1). The pipeline itself lands at M4.

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the layered pipeline (M4) is flex's first caller")
)]
mod flex;
