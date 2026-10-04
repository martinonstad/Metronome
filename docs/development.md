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

### Android app (one command)

```bash
cd android
. ../scripts/env.sh
./gradlew assembleDebug        # or installDebug to put it on a connected phone
```

The first build downloads Gradle, the Android Gradle Plugin and Compose (a few minutes);
later builds take seconds. Gradle runs `scripts/build-android-libs.sh` before every build, which:

1. cross-compiles `libmetronom_ffi.so` for `arm64-v8a` (about 360 KB) into
   `android/app/src/main/jniLibs/`, and
2. regenerates the Kotlin bindings into `android/app/src/main/java/uniffi/`.

Both outputs are git-ignored. Only `arm64-v8a` is built: every current phone, and the emulator
on Apple Silicon, uses it. A debug APK is about 12 MB because it is unminified and includes
Compose tooling; the 10 MB budget applies to the release build (Milestone 4).

### Kotlin bindings

The script generates them from the **host** build of the library, because release builds are
stripped of the metadata UniFFI needs. To run the steps by hand:

```bash
cd core
cargo build -p metronom-ffi
cargo run --features bindgen --bin uniffi-bindgen -- generate \
  --library target/debug/libmetronom_ffi.dylib \
  --language kotlin --out-dir ../android/app/src/main/java --no-format
```

`--no-format` skips the optional `ktlint` pass. The generated Kotlin depends on
[JNA](https://github.com/java-native-access/jna) (`com.sun.jna`), which the app includes as an
`aar` dependency. UniFFI exposes the Rust error type to Kotlin as `MetronomeException`.

**Naming rule for exported errors:** do not name an error variant's field `message` or `cause`.
The generated Kotlin exception inherits those from `Throwable`, and the build fails with
"Conflicting declarations".

## Running on a phone

1. On the phone: enable **Developer options**, then **USB debugging**.
2. Connect by USB and accept the "Allow USB debugging?" prompt.
3. `scripts/doctor.sh` should now list the device (or run `adb devices`).
4. Install and run: `cd android && . ../scripts/env.sh && ./gradlew installDebug`, then open
   **Metronom** on the phone. Debug builds log the audio diagnostics line every two seconds:
   `adb logcat -s Metronom`.

Use a physical phone for anything involving audio timing, latency or background playback;
emulators do not reproduce either.

## Release build

`scripts/check-size.sh` builds the release APK (R8 shrinks the code and resources) and fails if it
is 10 MB or larger; `scripts/test-all.sh` runs it. Without a signing key the APK is unsigned
(`android/app/build/outputs/apk/release/app-release-unsigned.apk`): good for measuring, not
installable. At the time of writing it is about 2.4 MB.

**Signing.** The release key is **never in the repository**; the build reads it from environment
variables. Make your own key once, keep the file and its password safe (a copy somewhere other than
this Mac), and lose neither: an app signed with a lost key can only be replaced by uninstalling it.

```bash
keytool -genkeypair -v -keystore ~/metronom-release.jks -alias metronom \
  -keyalg RSA -keysize 4096 -validity 10000
```

Then build a signed APK, or an Android App Bundle (what Google Play wants):

```bash
export METRONOM_KEYSTORE=~/metronom-release.jks
export METRONOM_KEYSTORE_PASSWORD='…'   # METRONOM_KEY_PASSWORD too, if the key's differs
export METRONOM_KEY_ALIAS=metronom
cd android && . ../scripts/env.sh && ./gradlew assembleRelease     # APK, to install by hand
cd android && . ../scripts/env.sh && ./gradlew bundleRelease       # AAB, for Google Play
$ANDROID_HOME/build-tools/36.0.0/apksigner verify --verbose android/app/build/outputs/apk/release/app-release.apk
```

A release build is signed differently from a debug build, so Android will not install one over
the other: uninstall first (this deletes the app's data; export your library before).

The R8 rules are in `android/app/proguard-rules.pro`. The Rust library is reached through JNA and
the Kotlin bindings UniFFI generates, which JNA finds by reflection, so those classes are kept.
After changing them, **install and run a signed release build on a phone**: a shrinking mistake
only shows up as a crash at run time, not as a build error.

## Project conventions

- **`metronom-core` has `#![forbid(unsafe_code)]`.** Only `metronom-ffi`'s audio module may use
  `unsafe`, and every use needs a `// SAFETY:` comment.
- **Real-time code** (the audio callback and anything it calls) must not allocate, lock, or do
  I/O. Communicate with it through atomics.
- **Clamp, don't reject.** Out-of-range user input is clamped to the valid range.
- **Formatting and lints are enforced:** `cargo fmt` and `cargo clippy -D warnings` run in
  `scripts/test-all.sh`.
- **Android application id.** It is `no.onstad.metronom`, a placeholder. It is permanent once an
  app is published, so change it (in `android/app/build.gradle.kts` and the Kotlin package)
  before the first release if it should be different.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `cargo: command not found` / `rustup-init: command not found` | Homebrew rustup is keg-only. `. scripts/env.sh`, or add `/opt/homebrew/opt/rustup/bin` and `~/.cargo/bin` to your `PATH` |
| `doctor.sh`: "java 25 is not 17 or 21" | `brew install openjdk@21`; `scripts/env.sh` selects it automatically |
| Gradle fails to configure with a JDK or "unsupported class file" error | Gradle is running on a JDK that is too new. Run via `scripts/env.sh`, or set `JAVA_HOME` to JDK 21 |
| `sdkmanager` prints a deprecation warning | Expected; Google is moving to the `android` CLI. It still works |
| `android` CLI prints a data-collection notice | It collects usage metrics by default. Pass `--no-metrics` (for example `android --no-metrics create --list`) |
| Kotlin: "Return type mismatch: expected 'com.sun.jna.Library'" in `metronom_ffi.kt` | An exported UniFFI object is named `Library`, which clashes with the JNA class the generated file imports. Give the Rust type another name (it is `SongLibrary`) |
| UniFFI bindgen finds no metadata in the `.so` | The release library is stripped. Generate from the host library as shown above |
| Kotlin: "Conflicting declarations" / "'message' hides member of supertype 'Throwable'" in `metronom_ffi.kt` | An exported Rust error variant has a field named `message` or `cause`. Rename it (see the naming rule above) |
| Gradle: "Unable to strip the following libraries" | Seen on debug builds and harmless there: the libraries are simply packaged unstripped (the Rust library is already size-optimised). Re-check on release builds in Milestone 4 |
| No device shown by `adb devices` | Re-plug, confirm the USB debugging prompt on the phone, try a data-capable cable; `adb kill-server && adb start-server` |
| Phone shows "unauthorized" | Revoke USB debugging authorisations in Developer options and reconnect |
