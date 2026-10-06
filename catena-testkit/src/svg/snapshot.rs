//! Visual snapshots: a frame's SVG and hash stored per name, and the check against them.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use catena::raster::CellGrid;

use super::{grid_to_hash, grid_to_svg};

/// Whether a snapshot check compares against the stored hash or replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Fail on a missing or different hash.
    Compare,
    /// Store the current frame's SVG and hash, and pass.
    Update,
}

impl Mode {
    /// `Update` when `CATENA_UPDATE_SNAPSHOTS` is `1` or `true`, else `Compare`.
    #[must_use]
    pub fn from_env() -> Mode {
        let update = std::env::var("CATENA_UPDATE_SNAPSHOTS")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if update { Mode::Update } else { Mode::Compare }
    }
}

/// Why a frame failed its snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// No `{name}.hash` is stored.
    Missing {
        /// The snapshot's name.
        name: String,
    },
    /// The stored hash differs from the frame's.
    Differs {
        /// The snapshot's name.
        name: String,
        /// The hash in `{name}.hash`.
        stored: String,
        /// The frame's hash.
        current: String,
    },
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mismatch::Missing { name } => write!(
                f,
                "\nNo visual snapshot '{name}' is stored yet.\n\
Current: {name}.fail.svg (inspect it)\n\
\n\
To accept it, rerun the test with CATENA_UPDATE_SNAPSHOTS=1.\n"
            ),
            Mismatch::Differs {
                name,
                stored,
                current,
            } => write!(
                f,
                "\nVisual snapshot mismatch for '{name}'.\n\
Reference: {name}.svg (inspect on GitHub)\n\
Failure:   {name}.fail.svg (current rendering)\n\
\n\
Reference hash: {stored}\n\
Current hash:   {current}\n\
\n\
To update the reference, rerun the test with CATENA_UPDATE_SNAPSHOTS=1.\n"
            ),
        }
    }
}

/// A directory of visual snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualSnapshots {
    dir: PathBuf,
}

impl VisualSnapshots {
    /// Snapshots stored in `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        VisualSnapshots { dir: dir.into() }
    }

    /// Snapshots stored in `tests/visual/` of the package under test, found through the
    /// `CARGO_MANIFEST_DIR` that `cargo test` sets.
    ///
    /// # Panics
    ///
    /// When run outside cargo, with `CARGO_MANIFEST_DIR` unset.
    #[must_use]
    pub fn for_package() -> Self {
        let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is unset: run visual snapshots under `cargo test`");
        VisualSnapshots::new(PathBuf::from(manifest).join("tests").join("visual"))
    }

    /// Where the snapshots live.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Compares `grid` against the stored `{name}.hash`, writing nothing.
    ///
    /// # Errors
    ///
    /// [`Mismatch`] when no hash is stored or it differs from the grid's.
    ///
    /// # Panics
    ///
    /// When `name` is not a plain file stem, or the stored hash cannot be read.
    pub fn check(&self, grid: &CellGrid, name: &str) -> Result<(), Mismatch> {
        let hash_path = self.path(name, "hash");
        if !hash_path.exists() {
            return Err(Mismatch::Missing {
                name: name.to_string(),
            });
        }
        let stored = fs::read_to_string(&hash_path).expect("read the stored snapshot hash");
        let stored = stored.trim();
        let current = grid_to_hash(grid);
        if current == stored {
            Ok(())
        } else {
            Err(Mismatch::Differs {
                name: name.to_string(),
                stored: stored.to_string(),
                current,
            })
        }
    }

    /// Checks `grid` as [`VisualSnapshots::check`] does, or in `Mode::Update` stores it. A
    /// failure writes `{name}.fail.svg`; a pass removes a stale one.
    ///
    /// # Errors
    ///
    /// [`Mismatch`] when `mode` is `Compare` and the check fails.
    ///
    /// # Panics
    ///
    /// When `name` is not a plain file stem, or a snapshot file cannot be read or written.
    pub fn verify(&self, grid: &CellGrid, name: &str, mode: Mode) -> Result<(), Mismatch> {
        let result = match mode {
            Mode::Update => {
                self.save(grid, name);
                fs::write(self.path(name, "hash"), grid_to_hash(grid))
                    .expect("write the snapshot hash");
                Ok(())
            }
            Mode::Compare => self.check(grid, name),
        };
        let fail_path = self.path(name, "fail.svg");
        if result.is_err() {
            fs::create_dir_all(&self.dir).expect("create the snapshot directory");
            let fail_svg = grid_to_svg(grid, &format!("{name} [FAIL]"));
            fs::write(&fail_path, fail_svg).expect("write the failure SVG");
        } else if fail_path.exists() {
            fs::remove_file(&fail_path).expect("remove the stale failure SVG");
        }
        result
    }

    /// Asserts that `grid` matches the stored snapshot `name`, or stores it when
    /// `CATENA_UPDATE_SNAPSHOTS=1` (see [`VisualSnapshots::verify`]).
    ///
    /// # Panics
    ///
    /// On a mismatch, with both hashes and how to update, after writing `{name}.fail.svg`.
    pub fn assert(&self, grid: &CellGrid, name: &str) {
        if let Err(mismatch) = self.verify(grid, name, Mode::from_env()) {
            panic!("{mismatch}");
        }
    }

    /// Writes `{name}.svg` and never compares: for a gallery of frames meant for eyes. Every
    /// `catena` frame is deterministic, so this is never a way around an unstable hash.
    ///
    /// # Panics
    ///
    /// When `name` is not a plain file stem, or the SVG cannot be written.
    pub fn save(&self, grid: &CellGrid, name: &str) {
        let path = self.path(name, "svg");
        fs::create_dir_all(&self.dir).expect("create the snapshot directory");
        fs::write(path, grid_to_svg(grid, name)).expect("write the snapshot SVG");
    }

    fn path(&self, name: &str, extension: &str) -> PathBuf {
        let plain = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        assert!(
            plain,
            "snapshot names are ASCII letters, digits, '_' and '-', got {name:?}"
        );
        self.dir.join(format!("{name}.{extension}"))
    }
}

/// Asserts that `grid` matches the snapshot `name` in the package's `tests/visual/`
/// ([`VisualSnapshots::for_package`], [`VisualSnapshots::assert`]).
///
/// # Panics
///
/// On a missing or different snapshot, after writing `{name}.fail.svg`.
pub fn assert_visual_snapshot(grid: &CellGrid, name: &str) {
    VisualSnapshots::for_package().assert(grid, name);
}

/// Writes `{name}.svg` to the package's `tests/visual/` without comparing
/// ([`VisualSnapshots::save`]).
///
/// # Panics
///
/// When the SVG cannot be written.
pub fn save_visual_gallery(grid: &CellGrid, name: &str) {
    VisualSnapshots::for_package().save(grid, name);
}
