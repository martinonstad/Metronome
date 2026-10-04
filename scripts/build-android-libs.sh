#!/usr/bin/env bash
# Builds libmetronom_ffi.so for Android (arm64-v8a) and regenerates the Kotlin bindings.
# Gradle runs this before every build (see android/app/build.gradle.kts); it can also be run
# by hand from anywhere.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/env.sh
. scripts/env.sh

jni_dir="$PWD/android/app/src/main/jniLibs"
kotlin_dir="$PWD/android/app/src/main/java"

cd core

echo "== Rust: Android library (arm64-v8a, release) =="
cargo ndk -t arm64-v8a -P 26 -o "$jni_dir" build --release -p metronom-ffi

# Bindings are generated from the host build: release libraries are stripped of the metadata
# UniFFI's library mode needs. The generated interface is identical on every target.
echo "== Kotlin bindings =="
case "$(uname -s)" in
  Darwin) host_lib=target/debug/libmetronom_ffi.dylib ;;
  *)      host_lib=target/debug/libmetronom_ffi.so ;;
esac
cargo build -p metronom-ffi
rm -rf "$kotlin_dir/uniffi"
cargo run --quiet --features bindgen --bin uniffi-bindgen -- generate \
  --library "$host_lib" --language kotlin --out-dir "$kotlin_dir" --no-format
