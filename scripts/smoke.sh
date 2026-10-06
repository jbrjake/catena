#!/usr/bin/env bash
# Commit gate (plan §17), run by .githooks/pre-commit. Under 60 s: fmt and `cargo xtask lint`
# over everything, then clippy and lib tests for the crates the staged diff touches. Quiet on
# success (one OK line); on failure it prints only what failed. Skips are announced, never
# silent. Run by hand with nothing staged, it scopes to the working tree's diff, and with no
# diff at all, to every crate.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
start=$(date +%s)
log=$(mktemp)
trap 'rm -f "$log"' EXIT

# Prints the part of the log worth reading: failing tests if any ran, else compiler output.
show_failure() {
    if grep -q '^failures:$' "$log"; then
        awk '/^failures:$/ { shown = 1 } shown' "$log"
    else
        grep -vE '^ *(Compiling|Checking|Finished|Running|Blocking|Downloaded|Downloading|Updating|Locking|Adding) ' "$log" || true
    fi
}

run_step() {
    local name=$1
    shift
    if ! "$@" >"$log" 2>&1; then
        echo "smoke: FAIL at $name: $*" >&2
        show_failure >&2
        exit 1
    fi
}

changed=$(git diff --cached --name-only)
if [ -z "$changed" ] && git rev-parse --verify --quiet HEAD >/dev/null; then
    changed=$(git diff --name-only HEAD)
fi

crates=""
add() {
    case " $crates " in
        *" $1 "*) ;;
        *) crates="$crates $1" ;;
    esac
}
if [ -z "$changed" ]; then
    echo "smoke: no diff, so every crate is in scope"
    crates=" catena catena-ratatui catena-testkit xtask"
fi
saved_ifs=$IFS
IFS=$'\n'
set -f
for path in $changed; do
    case "$path" in
        Cargo.toml | Cargo.lock | .cargo/*) crates=" catena catena-ratatui catena-testkit xtask" ;;
        catena/*) add catena; add catena-ratatui; add catena-testkit ;;
        catena-testkit/*) add catena-testkit; add catena ;;
        catena-ratatui/*) add catena-ratatui ;;
        xtask/*) add xtask ;;
    esac
done
set +f
IFS=$saved_ifs

run_step fmt cargo fmt --all -- --check
run_step lint cargo xtask lint

passed=0
if [ -z "$crates" ]; then
    echo "smoke: SKIP clippy and tests: the diff touches no crate"
else
    packages=""
    for crate in $crates; do
        packages="$packages -p $crate"
    done
    # shellcheck disable=SC2086 # $packages is a word list on purpose.
    run_step clippy cargo clippy $packages --all-targets -- -D warnings
    # shellcheck disable=SC2086
    run_step test cargo test $packages --lib
    passed=$(awk '/^test result: ok\./ { n += $4 } END { print n + 0 }' "$log")
fi

elapsed=$(($(date +%s) - start))
echo "smoke: OK, $passed tests passed (crates:${crates:- none}) in ${elapsed}s"
if [ "$elapsed" -gt 60 ]; then
    echo "smoke: WARNING: ${elapsed}s is over the 60 s budget; narrow what the gate runs" >&2
fi
