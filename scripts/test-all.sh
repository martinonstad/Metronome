#!/usr/bin/env bash
# Runs every automated check. Add new suites here as milestones land.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/env.sh
. scripts/env.sh

echo "== Rust: format =="
(cd core && cargo fmt --all -- --check)
echo "== Rust: clippy =="
(cd core && cargo clippy --all-targets -- -D warnings)
echo "== Rust: clippy for Android (compiles the Android-only audio code) =="
(cd core && cargo ndk -t arm64-v8a -P 26 clippy -p metronom-ffi -- -D warnings)
echo "== Rust: tests =="
(cd core && cargo test)
echo "== Android: debug build (also rebuilds the Rust library and Kotlin bindings) =="
(cd android && ./gradlew --console=plain --quiet assembleDebug)
echo "All checks passed."
