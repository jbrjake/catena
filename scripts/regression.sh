#!/usr/bin/env bash
# Push gate (plan §17), run by .githooks/pre-push, and the body of CI's `check` job, so a
# developer and CI see the same gate. Deterministic correctness only: fmt, `cargo xtask lint`,
# strict clippy, strict rustdoc and every test including ignored ones. No benchmarks, no fuzzing,
# no PTY tier. Warnings fail here, in the gate's own commands, never through build flags.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
start=$(date +%s)
log=$(mktemp)
trap 'rm -f "$log"' EXIT

show_failure() {
    if grep -q '^failures:$' "$log"; then
        awk '/^failures:$/ { shown = 1 } shown' "$log"
    else
        grep -vE '^ *(Compiling|Checking|Documenting|Finished|Running|Blocking|Downloaded|Downloading|Updating|Locking|Adding) ' "$log" || true
    fi
}

run_step() {
    local name=$1
    shift
    echo "regression: $name"
    if ! "$@" >"$log" 2>&1; then
        echo "regression: FAIL at $name: $*" >&2
        show_failure >&2
        exit 1
    fi
}

run_step fmt cargo fmt --all -- --check
run_step lint cargo xtask lint
run_step clippy cargo clippy --workspace --all-targets --all-features -- -D warnings
run_step doc env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
run_step test cargo test --workspace -- --include-ignored
passed=$(awk '/^test result: ok\./ { n += $4 } END { print n + 0 }' "$log")

echo "regression: OK, $passed tests passed in $(($(date +%s) - start))s"
