# Development guide

How to set up a machine, build, test and run Metronom. Commands assume macOS with Homebrew
(Apple Silicon). Run everything from the repository root.

## Quick check

```bash
scripts/doctor.sh      # what is installed, what is missing, and the fix for each gap
scripts/test-all.sh    # cargo fmt --check, clippy -D warnings, all Rust tests
```

`doctor.sh` exits non-zero if anything needed for Android development is missing. Xcode is
reported but only needed for the iPhone milestone.

## Toolchain setup

| Tool | Version used | Install |
|---|---|---|
| Rust (via rustup) | stable (1.99) | `brew install rustup && rustup default stable` |
| Android Rust targets | — | `rustup target add aarch64-linux-android x86_64-linux-android` |
| cargo-ndk | 4.x | `cargo install cargo-ndk --locked` |
| JDK | **21** | `brew install openjdk@21` |
| Android command-line tools + adb | — | `brew install --cask android-commandlinetools android-platform-tools` |
| Android SDK packages | platform 36, build-tools 36.0.0, NDK 29.0.14206865 | see below |
| Xcode | latest | Mac App Store (iPhone milestone only) |

Android SDK packages (this accepts Google's SDK licenses — read them first):

```bash
. scripts/env.sh
sdkmanager --licenses
sdkmanager "platforms;android-36" "build-tools;36.0.0" "ndk;29.0.14206865"
```

Android Studio is **not required**; the command-line tools are enough, and you can still open
the `android/` folder in Android Studio later.

### Environment

Homebrew's `rustup` and `openjdk@21` are "keg-only" and not on `PATH` by default. The scripts
source `scripts/env.sh`, which adds them, sets `JAVA_HOME` to JDK 21, and sets `ANDROID_HOME` to
the Homebrew SDK location. To use the same environment in your own shell:

```bash
. scripts/env.sh
```

### Why JDK 21, not the newest

The Android Gradle Plugin does not support every new JDK. JDK 21 is a safe, long-term-support
choice. `doctor.sh` warns if the active Java is not 17 or 21.

## Building

### Rust core (host)

```bash
cd core
cargo test                    # unit tests
cargo clippy --all-targets -- -D warnings
```

### Android native library

```bash
. scripts/env.sh
cd core
cargo ndk -t arm64-v8a -P 26 -o ../android/app/src/main/jniLibs build --release -p metronom-ffi
```

This writes `libmetronom_ffi.so` (about 360 KB) to `android/app/src/main/jniLibs/arm64-v8a/`.
Only `arm64-v8a` is built: every current phone, and the emulator on Apple Silicon, uses it.

The Android app shell and the Gradle task that runs this automatically are part of Milestone 0 and
are not committed yet.

### Kotlin bindings

Generated from the **host** build, because release builds are stripped of the metadata UniFFI
needs:

```bash
cd core
cargo build -p metronom-ffi
cargo run --features bindgen --bin uniffi-bindgen -- generate \
  --library target/debug/libmetronom_ffi.dylib \
  --language kotlin --out-dir ../android/app/src/main/java --no-format
```

Generated files are git-ignored and are recreated by the build. `--no-format` skips the optional
`ktlint` pass. The generated Kotlin depends on [JNA](https://github.com/java-native-access/jna)
(`com.sun.jna`), which the Android app must include as an `aar` dependency; UniFFI exposes the
Rust error type to Kotlin as `MetronomeException`.

## Running on a phone

1. On the phone: enable **Developer options**, then **USB debugging**.
2. Connect by USB and accept the "Allow USB debugging?" prompt.
3. `scripts/doctor.sh` should now list the device (or run `adb devices`).
4. Install and run (once the app shell exists): `cd android && ./gradlew installDebug`.

Use a physical phone for anything involving audio timing, latency or background playback;
emulators do not reproduce either.

## Project conventions

- **`metronom-core` has `#![forbid(unsafe_code)]`.** Only `metronom-ffi`'s audio module may use
  `unsafe`, and every use needs a `// SAFETY:` comment.
- **Real-time code** (the audio callback and anything it calls) must not allocate, lock, or do
  I/O. Communicate with it through atomics.
- **Clamp, don't reject.** Out-of-range user input is clamped to the valid range.
- **Formatting and lints are enforced:** `cargo fmt` and `cargo clippy -D warnings` run in
  `scripts/test-all.sh`.
- **Android application id.** The planned id is `no.onstad.metronom`, a placeholder that is not
  in the repository yet. It is permanent once an app is published, so confirm it before the
  Android project is committed.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `cargo: command not found` / `rustup-init: command not found` | Homebrew rustup is keg-only. `. scripts/env.sh`, or add `/opt/homebrew/opt/rustup/bin` and `~/.cargo/bin` to your `PATH` |
| `doctor.sh`: "java 25 is not 17 or 21" | `brew install openjdk@21`; `scripts/env.sh` selects it automatically |
| Gradle fails to configure with a JDK or "unsupported class file" error | Gradle is running on a JDK that is too new. Run via `scripts/env.sh`, or set `JAVA_HOME` to JDK 21 |
| `sdkmanager` prints a deprecation warning | Expected; Google is moving to the `android` CLI. It still works |
| `android` CLI prints a data-collection notice | It collects usage metrics by default. Pass `--no-metrics` (for example `android --no-metrics create --list`) |
| UniFFI bindgen finds no metadata in the `.so` | The release library is stripped. Generate from the host library as shown above |
| No device shown by `adb devices` | Re-plug, confirm the USB debugging prompt on the phone, try a data-capable cable; `adb kill-server && adb start-server` |
| Phone shows "unauthorized" | Revoke USB debugging authorisations in Developer options and reconnect |
