// Visual snapshot assertions for the Cylvia TUI.
//
// Snapshots are stored as two files per named test case:
//
//   tests/visual/snapshots/{name}.svg   — full-colour SVG for GitHub inspection
//   tests/visual/snapshots/{name}.hash  — 64-char SHA-256 hex for comparison
//
// On first run (no reference files present) the snapshot is saved and the test
// passes — this lets the CI green-light the initial capture. On subsequent
// runs the SHA-256 hash is compared; a mismatch panics with an actionable
// message pointing to both the reference SVG and the newly rendered failure SVG.
//
// Set `CYLVIA_UPDATE_SNAPSHOTS=1` to unconditionally overwrite the reference
// files (useful after intentional UI changes).

use std::path::PathBuf;

use super::harness::TuiTestRunner;
use super::svg_renderer::{buffer_to_hash, buffer_to_svg};

// ── Public API ────────────────────────────────────────────────────────────────

/// Save a visual snapshot for gallery / documentation purposes only.
///
/// Like `assert_visual_snapshot`, but **never fails on hash mismatch** —
/// it always overwrites the reference with the current frame.
///
/// Use this for screens whose rendered output is inherently non-deterministic
/// across process runs (e.g. graph canvases whose node positions depend on
/// `HashMap` iteration order in the FR layout algorithm).  The SVG is still
/// committed and inspectable on GitHub; it just doesn't gate CI.
pub fn save_visual_gallery(runner: &TuiTestRunner, name: &str) {
    save_snapshot(name, runner);
}

/// Assert that the current rendered buffer matches the stored visual snapshot
/// for `name`.
///
/// Behaviour:
/// - **First run** (no `.hash` reference): saves the SVG and hash, passes.
/// - **Subsequent runs**: compares the SHA-256 hash; panics on mismatch.
/// - **`CYLVIA_UPDATE_SNAPSHOTS=1`**: always saves and passes (re-baselines).
///
/// On mismatch, a `{name}.fail.svg` is written alongside the reference files
/// so the failure can be visually inspected without running the test again.
///
/// Only use this for screens whose output is fully deterministic across runs
/// (overlays, forms, and modals that completely cover the graph canvas).
/// For screens containing the graph, use `save_visual_gallery` instead.
pub fn assert_visual_snapshot(runner: &TuiTestRunner, name: &str) {
    let buf = runner.buffer();

    let update = std::env::var("CYLVIA_UPDATE_SNAPSHOTS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let hash_path = snapshot_dir().join(format!("{name}.hash"));

    if update || !hash_path.exists() {
        // Save new reference — either forced update or first run.
        save_snapshot(name, runner);
        return;
    }

    // Compare against the stored hash.
    match compare_snapshot(name, runner) {
        Ok(()) => {} // All good.
        Err(msg) => {
            // Write a failure SVG so the diff can be inspected visually.
            let fail_svg = buffer_to_svg(buf, &format!("{name} [FAIL]"));
            let fail_path = snapshot_dir().join(format!("{name}.fail.svg"));
            std::fs::write(&fail_path, fail_svg.as_bytes()).expect("failed to write failure SVG");
            panic!("{msg}");
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Return the canonical snapshot storage directory.
///
/// Always `{CARGO_MANIFEST_DIR}/tests/visual/snapshots/`.  Using the manifest
/// dir (set at compile time) means the path is correct regardless of the
/// working directory when `cargo test` runs.
fn snapshot_dir() -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest)
        .join("tests")
        .join("visual")
        .join("snapshots")
}

/// Write the `.svg` and `.hash` reference files for `name`.
fn save_snapshot(name: &str, runner: &TuiTestRunner) {
    let buf = runner.buffer();
    let dir = snapshot_dir();

    std::fs::create_dir_all(&dir).expect("failed to create snapshot directory");

    let svg = buffer_to_svg(buf, name);
    std::fs::write(dir.join(format!("{name}.svg")), svg.as_bytes())
        .expect("failed to write snapshot SVG");

    let hash = buffer_to_hash(buf);
    std::fs::write(dir.join(format!("{name}.hash")), hash.as_bytes())
        .expect("failed to write snapshot hash");
}

/// Compare the current buffer against the stored `.hash` file.
///
/// Returns `Ok(())` when hashes match, or `Err(message)` with a human-readable
/// explanation and instructions for updating the snapshot.
fn compare_snapshot(name: &str, runner: &TuiTestRunner) -> Result<(), String> {
    let buf = runner.buffer();
    let dir = snapshot_dir();

    let hash_path = dir.join(format!("{name}.hash"));
    let stored_hash = std::fs::read_to_string(&hash_path)
        .expect("hash file should exist — checked before calling compare_snapshot");
    let stored_hash = stored_hash.trim();

    let current_hash = buffer_to_hash(buf);

    if current_hash == stored_hash {
        return Ok(());
    }

    Err(format!(
        "\n\
Visual snapshot mismatch for '{name}'.\n\
Reference: tests/visual/snapshots/{name}.svg (inspect on GitHub)\n\
Failure:   tests/visual/snapshots/{name}.fail.svg (current rendering)\n\
\n\
Reference hash: {stored_hash}\n\
Current hash:   {current_hash}\n\
\n\
To update the reference run:\n\
  CYLVIA_UPDATE_SNAPSHOTS=1 cargo test -p nexus-tui -F visual-tests --test visual {name}\n"
    ))
}
