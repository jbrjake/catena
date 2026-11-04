//! T4 cross-run determinism (plan §16.5, §11.5): the same fixture drawn by two fresh processes
//! hashes alike, and as committed, so CI's three platforms are held to one frame.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use catena::raster::Blitter;
use catena_testkit::fixtures::{community, generated};
use catena_testkit::frames::{MAX_GENERATED_NODES, blitter, fixture, frame};
use catena_testkit::svg::grid_to_hash;

/// The frames the bin is run on: fixture, size, blitter.
const CASES: [(&str, u16, u16, &str); 5] = [
    ("community", 120, 40, "braille"),
    ("community", 120, 40, "ascii"),
    ("generated:7:120", 120, 40, "braille"),
    ("generated:3:60", 80, 24, "halfblock"),
    ("generated:11:40", 100, 30, "sextant"),
];

#[test]
fn fixture_and_blitter_names_parse_and_bad_ones_do_not() {
    assert_eq!(fixture("community"), Some(community()));
    assert_eq!(fixture("generated:5:30"), Some(generated(5, 30)));
    let ceiling = format!("generated:1:{MAX_GENERATED_NODES}");
    assert!(fixture(&ceiling).is_some());
    let past = format!("generated:1:{}", MAX_GENERATED_NODES + 1);
    for bad in [
        "",
        "nope",
        "generated",
        "generated:1",
        "generated:x:5",
        "generated:1:-3",
        "generated:1:5:9",
        &past,
    ] {
        assert_eq!(fixture(bad), None, "{bad:?}");
    }
    for (name, expected) in [
        ("braille", Blitter::Braille),
        ("halfblock", Blitter::HalfBlock),
        ("sextant", Blitter::Sextant),
        ("ascii", Blitter::Ascii),
    ] {
        assert_eq!(blitter(name), Some(expected));
    }
    assert_eq!(blitter("Braille"), None);
}

/// The hash the bin prints for `case`, from a fresh process.
fn run((name, cols, rows, blitter): (&str, u16, u16, &str)) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_catena-render-hash"))
        .args([name, &cols.to_string(), &rows.to_string(), blitter])
        .output()
        .expect("the bin runs");
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("a hex hash")
        .trim()
        .to_string()
}

fn committed(case: (&str, u16, u16, &str)) -> PathBuf {
    let (name, cols, rows, blitter) = case;
    let stem = format!("{}_{cols}x{rows}_{blitter}", name.replace(':', "_"));
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("hashes")
        .join(format!("{stem}.hash"))
}

#[test]
fn two_processes_draw_every_case_alike_and_as_committed() {
    let update = std::env::var_os("CATENA_UPDATE_SNAPSHOTS").is_some_and(|v| v == "1");
    for case in CASES {
        let (first, second) = (run(case), run(case));
        assert_eq!(first.len(), 64, "{case:?}: {first:?}");
        assert_eq!(
            first, second,
            "{case:?}: two processes drew different frames"
        );

        let (name, cols, rows, blit) = case;
        let graph = fixture(name).expect("a named fixture");
        let here = frame(&graph, blitter(blit).expect("a blitter"), (cols, rows))
            .expect("fixtures are consistent");
        assert_eq!(
            grid_to_hash(&here),
            first,
            "{case:?}: the bin draws as the library does"
        );

        let path = committed(case);
        if update {
            fs::create_dir_all(path.parent().expect("a directory")).expect("create tests/hashes");
            fs::write(&path, format!("{first}\n")).expect("write the hash");
        }
        let stored = fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("{case:?}: no hash at {path:?}; CATENA_UPDATE_SNAPSHOTS=1 stores it")
        });
        assert_eq!(
            stored.trim(),
            first,
            "{case:?}: the frame changed from the committed one"
        );
    }
}

#[test]
fn the_bin_refuses_bad_arguments() {
    for args in [
        vec![],
        vec!["community", "120", "40"],
        vec!["nope", "120", "40", "braille"],
        vec!["community", "wide", "40", "braille"],
        vec!["community", "120", "40", "plaid"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_catena-render-hash"))
            .args(&args)
            .output()
            .expect("the bin runs");
        assert!(!output.status.success(), "{args:?} succeeded");
        assert!(output.stdout.is_empty(), "{args:?} printed a hash");
    }
}
