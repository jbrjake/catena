// Blink detection tests — core infrastructure, steady-state stability,
// render determinism, and dirty flag tracking.
//
// These tests use the real engine (MemoryBackend → HTTP API → App → render)
// and the TuiTestRunner's frame capture to detect buffer changes between
// consecutive renders of identical state. Any non-zero diff = blink.
//
// The harness tick() now mirrors main.rs run_app() exactly, including all
// fetch_pending_* and poll_* calls. This catches regressions caused by
// background data-fetch functions that set dirty = true.

use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

use std::io::Write;
use std::sync::{Arc, Mutex};

use super::assertions::dump_screen;
use super::harness::TuiTestRunner;

// ── Shared write buffer for CrosstermBackend capture ─────────────────────

/// A writer that captures all bytes to a shared buffer.
/// Used to exercise the real CrosstermBackend escape sequence path.
#[derive(Clone)]
pub(super) struct CaptureBuf(Arc<Mutex<Vec<u8>>>);

impl CaptureBuf {
    pub(super) fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    /// Number of bytes written so far.
    pub(super) fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// Snapshot and clear the buffer, returning bytes written since last drain.
    pub(super) fn drain(&self) -> Vec<u8> {
        let mut buf = self.0.lock().unwrap();
        let data = buf.clone();
        buf.clear();
        data
    }
}

impl Write for CaptureBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Compare two buffers cell-by-cell and return detailed diff info.
pub(super) fn buffer_diff_details(a: &Buffer, b: &Buffer) -> Vec<String> {
    let updates = a.diff(b);
    updates
        .iter()
        .take(20)
        .map(|(x, y, cell)| {
            let old_cell = &a[(*x, *y)];
            format!(
                "({x},{y}): {:?}→{:?} fg:{:?}→{:?} bg:{:?}→{:?} mod:{:?}→{:?}",
                old_cell.symbol(),
                cell.symbol(),
                old_cell.fg,
                cell.fg,
                old_cell.bg,
                cell.bg,
                old_cell.modifier,
                cell.modifier,
            )
        })
        .collect()
}

/// Count of differing cells between two buffers.
pub(super) fn diff_count(a: &Buffer, b: &Buffer) -> usize {
    a.diff(b).len()
}

/// Scan a buffer for cells with SLOW_BLINK modifier.
pub(super) fn scan_slow_blink(buf: &Buffer) -> Vec<String> {
    let mut cells = Vec::new();
    for y in buf.area.y..buf.area.y + buf.area.height {
        for x in buf.area.x..buf.area.x + buf.area.width {
            let cell = &buf[(x, y)];
            if cell.modifier.contains(Modifier::SLOW_BLINK) {
                cells.push(format!(
                    "({x},{y}): {:?} fg={:?} mod={:?}",
                    cell.symbol(),
                    cell.fg,
                    cell.modifier
                ));
            }
        }
    }
    cells
}

// ── Steady-state frame stability after demo load ────────────────────────

/// Load the STIX demo (fast, no LLM), wait for stability, then verify
/// that 50 consecutive background-only ticks produce zero frame diffs.
///
/// This is the primary blink regression test. It exercises the exact same
/// code path as the real TUI event loop — every fetch_pending, poll, and
/// animation tick — and checks that none of them cause spurious redraws
/// or non-deterministic rendering.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn steady_state_no_blink_after_stix_demo() {
    let mut runner = TuiTestRunner::new(160, 50).await;
    runner.load_demo("stix-cyber").await;

    // Ensure demo loaded successfully.
    assert!(
        !runner.app().entities.is_empty(),
        "Demo should have loaded entities"
    );

    // Let any trailing async work settle (community name fetch, etc.)
    for _ in 0..20 {
        runner.tick();
    }

    // Establish baseline: two renders to prime ratatui's double buffer.
    let _ = runner.force_render();
    let baseline = runner.force_render();

    // Run 50 background-only ticks (no user input). Each tick calls every
    // fetch_pending/poll function exactly as main.rs does. If ANY tick
    // causes a render (dirty = true) or the buffer changes, it's a blink.
    let mut blink_ticks: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut prev = baseline.clone();

    for tick_num in 0..50 {
        let was_dirty = runner.tick_background_only();

        let current = runner.buffer_snapshot();
        let diffs = diff_count(&prev, &current);

        if diffs > 0 || was_dirty {
            let details = buffer_diff_details(&prev, &current);
            blink_ticks.push((tick_num, diffs, details));
        }
        prev = current;
    }

    assert!(
        blink_ticks.is_empty(),
        "BLINK DETECTED in steady state: {} of 50 ticks had unexpected changes.\n\
         Ticks with changes:\n{}",
        blink_ticks.len(),
        blink_ticks
            .iter()
            .map(|(tick, count, details)| {
                let dirty_str = if *count == 0 {
                    "dirty set (no buffer diff)"
                } else {
                    "buffer diff"
                };
                format!(
                    "  tick {tick}: {count} cells changed ({dirty_str})\n    {}",
                    details.join("\n    ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Same test but for the threat-intel dataset (LLM-based extraction with
/// replay fixtures). This dataset triggers community detection, embeddings,
/// and other enrichments that set pending flags.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn steady_state_no_blink_after_threat_intel_demo() {
    let mut runner = TuiTestRunner::new(160, 50).await;
    runner.load_demo("threat-intel").await;

    assert!(
        !runner.app().entities.is_empty(),
        "Demo should have loaded entities"
    );

    // Extra settling for enrichment-triggered fetches.
    for _ in 0..30 {
        runner.tick();
    }

    let _ = runner.force_render();
    let baseline = runner.force_render();

    let mut blink_ticks: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut prev = baseline;

    for tick_num in 0..50 {
        let was_dirty = runner.tick_background_only();
        let current = runner.buffer_snapshot();
        let diffs = diff_count(&prev, &current);

        if diffs > 0 || was_dirty {
            let details = buffer_diff_details(&prev, &current);
            blink_ticks.push((tick_num, diffs, details));
        }
        prev = current;
    }

    assert!(
        blink_ticks.is_empty(),
        "BLINK DETECTED in steady state (threat-intel): {} ticks with changes.\n{}",
        blink_ticks.len(),
        blink_ticks
            .iter()
            .map(|(tick, count, details)| format!(
                "  tick {tick}: {count} cells\n    {}",
                details.join("\n    ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

// ── Render determinism ──────────────────────────────────────────────────

/// Verify that re-rendering identical app state produces an identical
/// buffer. If this fails, the render function itself is non-deterministic.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn render_determinism_after_demo_load() {
    let mut runner = TuiTestRunner::new(160, 50).await;
    runner.load_demo("stix-cyber").await;

    for _ in 0..10 {
        runner.tick();
    }

    // Render 3 consecutive frames with no state changes.
    let frame1 = runner.force_render();
    let frame2 = runner.force_render();
    let frame3 = runner.force_render();

    let diffs_1_2 = diff_count(&frame1, &frame2);
    let diffs_2_3 = diff_count(&frame2, &frame3);

    assert_eq!(
        diffs_1_2,
        0,
        "Frames 1→2 differ by {} cells (non-deterministic render):\n{}",
        diffs_1_2,
        buffer_diff_details(&frame1, &frame2).join("\n  "),
    );
    assert_eq!(
        diffs_2_3,
        0,
        "Frames 2→3 differ by {} cells (non-deterministic render):\n{}",
        diffs_2_3,
        buffer_diff_details(&frame2, &frame3).join("\n  "),
    );
}

// ── Dirty flag should not be set in steady state ────────────────────────

/// After demo loading completes and the screen stabilizes, no background
/// tick should set dirty = true. If dirty is set, it means a fetch_pending
/// function is firing when it shouldn't, causing an unnecessary redraw.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_spurious_dirty_in_steady_state() {
    let mut runner = TuiTestRunner::new(160, 50).await;
    runner.load_demo("stix-cyber").await;

    // Settle all pending work.
    for _ in 0..30 {
        runner.tick();
    }

    // Verify dirty is currently false.
    assert!(
        !runner.app().dirty,
        "App should not be dirty after settling"
    );

    // Run 100 background ticks and check that dirty never gets set.
    let mut dirty_ticks = Vec::new();
    for tick_num in 0..100 {
        let was_dirty = runner.tick_background_only();
        if was_dirty {
            dirty_ticks.push(tick_num);
        }
    }

    assert!(
        dirty_ticks.is_empty(),
        "dirty was set on {} of 100 steady-state ticks: {:?}.\n\
         This means a fetch_pending function is firing when it shouldn't.\n\
         Screen:\n{}",
        dirty_ticks.len(),
        dirty_ticks,
        dump_screen(&runner),
    );
}

// ── Frame stability with user interaction ───────────────────────────────

/// After user interaction (select entity, view provenance, return), the
/// screen should stabilize within a few frames and not blink.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_blink_after_user_interaction() {
    let mut runner = TuiTestRunner::new(160, 50).await;
    runner.load_demo("stix-cyber").await;

    for _ in 0..10 {
        runner.tick();
    }

    // Select next entity.
    runner.send_char('n');
    runner.tick_all();

    // Open provenance.
    runner.send_char('p');
    for _ in 0..50 {
        runner.tick();
    }

    // Close provenance.
    runner.send_key(crossterm::event::KeyCode::Esc);
    runner.tick_all();

    // Now in Normal mode — let pending fetches complete.
    for _ in 0..20 {
        runner.tick();
    }

    // Establish baseline.
    let _ = runner.force_render();
    let baseline = runner.force_render();

    // 30 steady-state ticks.
    let mut blink_ticks = Vec::new();
    let mut prev = baseline;

    for tick_num in 0..30 {
        let was_dirty = runner.tick_background_only();
        let current = runner.buffer_snapshot();
        let diffs = diff_count(&prev, &current);

        if diffs > 0 || was_dirty {
            blink_ticks.push((tick_num, diffs));
        }
        prev = current;
    }

    assert!(
        blink_ticks.is_empty(),
        "BLINK DETECTED after user interaction: {:?}",
        blink_ticks,
    );
}

// ── Blink detection DURING loading ──────────────────────────────────────

/// Monitor every single frame DURING threat-intel demo loading.
///
/// On each tick, we check that the tick's own render was self-consistent
/// by forcing a second render and comparing. If the second render differs,
/// something in the tick changed state between the render and the re-render,
/// or the render itself has a non-idempotent side effect.
///
/// We also track dirty flag activity to identify which fetch_pending
/// functions are causing redraws during and after loading.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_blink_during_threat_intel_loading() {
    let mut runner = TuiTestRunner::new(160, 50).await;

    // Start demo loading (non-blocking) via HTTP worker channel.
    runner.setup_demo_loading("threat-intel");

    // Monitor every tick during loading.
    let mut non_deterministic_frames: Vec<(usize, usize, Vec<String>)> = Vec::new();

    for tick_num in 0..300 {
        // Tick processes fetch_pending/poll/animations + renders if dirty.
        runner.tick();

        // Now immediately force TWO consecutive renders (no state changes).
        // If render is deterministic and no state changes occur between
        // them, they must be identical.
        let frame_a = runner.force_render();
        let frame_b = runner.force_render();

        // Check render determinism: consecutive renders must match.
        let stability_diffs = diff_count(&frame_a, &frame_b);
        if stability_diffs > 0 {
            let details = buffer_diff_details(&frame_a, &frame_b);
            non_deterministic_frames.push((tick_num, stability_diffs, details));
        }

        // Stop early if loading is done and screen is stable.
        if !runner.app().demo_loading && !runner.app().entities.is_empty() {
            // Give 20 more ticks to catch post-load blinks.
            for extra_tick in 0..20 {
                runner.tick();
                let a = runner.force_render();
                let b = runner.force_render();
                let diffs = diff_count(&a, &b);
                if diffs > 0 {
                    let details = buffer_diff_details(&a, &b);
                    non_deterministic_frames.push((tick_num + extra_tick + 1, diffs, details));
                }
            }
            break;
        }
    }

    assert!(
        non_deterministic_frames.is_empty(),
        "NON-DETERMINISTIC RENDER during loading: {} frames rendered differently on re-render.\n{}",
        non_deterministic_frames.len(),
        non_deterministic_frames
            .iter()
            .take(10)
            .map(|(tick, count, details)| format!(
                "  tick {tick}: {count} cells differ\n    {}",
                details.join("\n    ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

// ── SLOW_BLINK attribute scanner with real engine ───────────────────────

/// Scan the rendered buffer for any cells with SLOW_BLINK modifier after
/// loading the threat-intel demo and entering various modes.
///
/// This catches SLOW_BLINK attributes that only appear when rendering with
/// real data (e.g., community colors, enrichment badges, etc.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_slow_blink_attribute_in_any_mode_with_real_data() {
    let mut runner = TuiTestRunner::new(160, 50).await;
    runner.load_demo("stix-cyber").await;

    for _ in 0..10 {
        runner.tick();
    }

    // Check normal mode.
    let blink_cells = scan_slow_blink(runner.buffer());
    assert!(
        blink_cells.is_empty(),
        "SLOW_BLINK in Normal mode with data: {}",
        blink_cells.join(", ")
    );

    // Select an entity and check.
    runner.send_char('n');
    runner.tick_all();
    let blink_cells = scan_slow_blink(runner.buffer());
    assert!(
        blink_cells.is_empty(),
        "SLOW_BLINK after entity selection: {}",
        blink_cells.join(", ")
    );

    // Enter search mode.
    runner.send_char('/');
    runner.tick_all();
    let blink_cells = scan_slow_blink(runner.buffer());
    assert!(
        blink_cells.is_empty(),
        "SLOW_BLINK in Search mode: {}",
        blink_cells.join(", ")
    );
    runner.send_key(crossterm::event::KeyCode::Esc);
    runner.tick_all();

    // Enter annotations mode.
    runner.send_char('a');
    runner.tick_all();
    for _ in 0..20 {
        runner.tick(); // wait for annotation fetch
    }
    let blink_cells = scan_slow_blink(runner.buffer());
    assert!(
        blink_cells.is_empty(),
        "SLOW_BLINK in Annotations mode: {}",
        blink_cells.join(", ")
    );
    runner.send_key(crossterm::event::KeyCode::Esc);
    runner.tick_all();

    // Enter help mode.
    runner.send_char('?');
    runner.tick_all();
    let blink_cells = scan_slow_blink(runner.buffer());
    assert!(
        blink_cells.is_empty(),
        "SLOW_BLINK in Help mode: {}",
        blink_cells.join(", ")
    );
}

// ── Dirty flag tracking during loading ──────────────────────────────────

/// Track exactly WHEN dirty gets set during threat-intel loading.
/// This helps identify which fetch_pending function is causing redraws.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dirty_flag_audit_during_loading() {
    let mut runner = TuiTestRunner::new(160, 50).await;

    runner.setup_demo_loading("threat-intel");

    // Track dirty events.
    let mut dirty_events: Vec<(usize, usize, String)> = Vec::new(); // (tick, entity_count, status)

    for tick_num in 0..300 {
        runner.tick();

        if runner.app().dirty {
            // dirty was set but tick didn't draw (this shouldn't happen normally)
            let entity_count = runner.app().entities.len();
            let status = runner.app().status_message.clone().unwrap_or_default();
            dirty_events.push((tick_num, entity_count, status));
        }

        if !runner.app().demo_loading && !runner.app().entities.is_empty() {
            break;
        }
    }

    // After loading, check for spurious dirty in steady state.
    let mut post_load_dirty = Vec::new();
    for tick_num in 0..50 {
        let was_dirty = runner.tick_background_only();
        if was_dirty {
            let entity_count = runner.app().entities.len();
            let status = runner.app().status_message.clone().unwrap_or_default();
            post_load_dirty.push((tick_num, entity_count, status));
        }
    }

    // Post-load dirty should be empty.
    assert!(
        post_load_dirty.is_empty(),
        "SPURIOUS DIRTY after loading: {} events.\n{}",
        post_load_dirty.len(),
        post_load_dirty
            .iter()
            .map(|(tick, count, status)| format!(
                "  tick {tick}: {count} entities, status={status:?}"
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
