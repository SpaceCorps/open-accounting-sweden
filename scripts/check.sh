#!/usr/bin/env bash
# Every check a PR must pass, run locally (there is no hosted CI). Needs: cargo, vp, curl, sqlite3, python3.
# The web checks need node_modules in web/ (see the README's web app section).
# Usage: scripts/check.sh
set -eu
cd "$(dirname "$0")/.."
step() { echo; echo "== $1"; }
step "cargo fmt";    cargo fmt --all --check
step "cargo clippy"; cargo clippy --workspace --all-targets -q -- -D warnings
step "cargo test";   cargo test --workspace -q
step "web check";    (cd web && vp check)
step "web build";    (cd web && vp build >/dev/null)
step "smoke";        scripts/smoke.sh
echo; echo "all checks passed"
