//! Geometry: node measurement, grid snapping and the viewport (plan §5, §6).

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "ResolvedMetrics and the viewport (M2) are its first callers"
    )
)]
mod zoom;
