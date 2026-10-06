//! Zoom utilities — semantic zoom level mapping and visual label width scaling.
//!
//! Extracted from `graph/mod.rs` to keep the main graph module focused on
//! layout orchestration.

/// Map continuous zoom level (0.1–4.0+) to semantic zoom level (0–5).
/// See PRD-021 §3.1.5.
///
/// Labels appear sooner (level 2 at zoom 0.35) and grow progressively
/// longer at higher zoom levels, reaching full untruncated names at level 5.
pub(crate) fn zoom_to_semantic(zoom: f32) -> u8 {
    if zoom <= 0.3 {
        0 // symbol only (●, ■, ▲)
    } else if zoom <= 0.35 {
        1 // abbreviated [ABC]
    } else if zoom <= 1.5 {
        2 // short full name (~12 chars)
    } else if zoom <= 2.5 {
        3 // medium name (~20 chars)
    } else if zoom <= 3.5 {
        4 // long name (~32 chars)
    } else {
        5 // full name, no truncation
    }
}

/// Compute the effective visual label width at a given semantic zoom level.
///
/// The layout engine stores `label_width` based on the full entity name,
/// but at lower zoom levels the rendered label is shorter. Edge endpoints
/// must use the visual width so lines connect to the visible label center.
/// Caps include 2 chars for brackets: level 2 cap of 14 → ~12 char name.
pub(crate) fn visual_label_width(layout_width: u16, zoom_level: u8) -> u16 {
    match zoom_level {
        0 => 1,
        1 => layout_width.min(5),
        2 => layout_width.min(14),
        3 => layout_width.min(22),
        4 => layout_width.min(34),
        _ => layout_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_zoom_levels() {
        assert_eq!(zoom_to_semantic(0.1), 0);
        assert_eq!(zoom_to_semantic(0.3), 0);
        // Abbreviated labels appear at zoom 0.31–0.35
        assert_eq!(zoom_to_semantic(0.35), 1);
        // Full labels (short) appear sooner: at zoom 0.36+
        assert_eq!(zoom_to_semantic(0.5), 2);
        assert_eq!(zoom_to_semantic(0.75), 2);
        assert_eq!(zoom_to_semantic(1.0), 2);
        // Progressive levels at high zoom
        assert_eq!(zoom_to_semantic(2.0), 3);
        assert_eq!(zoom_to_semantic(3.0), 4);
        assert_eq!(zoom_to_semantic(4.0), 5);
    }

    #[test]
    fn visual_label_width_by_zoom() {
        // Zoom 0 (symbol): always 1 character regardless of name length
        assert_eq!(visual_label_width(22, 0), 1);
        assert_eq!(visual_label_width(3, 0), 1);

        // Zoom 1 (abbreviated [ABC]): capped at 5, but respects short names
        assert_eq!(visual_label_width(22, 1), 5);
        assert_eq!(visual_label_width(3, 1), 3);
        assert_eq!(visual_label_width(5, 1), 5);

        // Zoom 2 (short full name): capped at 14, ~12 char names + brackets
        assert_eq!(visual_label_width(22, 2), 14);
        assert_eq!(visual_label_width(3, 2), 3);
        assert_eq!(visual_label_width(14, 2), 14);

        // Zoom 3 (medium name): capped at 22
        assert_eq!(visual_label_width(30, 3), 22);
        assert_eq!(visual_label_width(10, 3), 10);

        // Zoom 4 (long name): capped at 34
        assert_eq!(visual_label_width(40, 4), 34);
        assert_eq!(visual_label_width(20, 4), 20);

        // Zoom 5+ (full name): uses layout width as-is
        assert_eq!(visual_label_width(50, 5), 50);
        assert_eq!(visual_label_width(3, 5), 3);
    }
}
