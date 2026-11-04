#!/usr/bin/env bash
# The performance ratchet (plan §15.1): runs catena's bench target, holds every budgeted bench to
# its plan §15.1 budget, and holds every bench and the peak RSS to 1.5× of this machine's entry
# in the committed bench-baseline.json. A regression past that needs the owner's approval; then
# UPDATE_BASELINE=1 reseeds the entry. A machine with no entry seeds one loudly. On demand and in
# CI's bench job only, never in a git hook. The logic lives in `cargo xtask perf`.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
exec cargo xtask perf "$@"
