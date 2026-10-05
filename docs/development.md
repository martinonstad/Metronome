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

**Signing.** The release key is **never in the repository** and is never typed into anything but
your own terminal. Make it once, yourself:

```bash
scripts/create-release-key.sh
```

`keytool` asks you for a password (nothing is shown as you type). The key goes to
`~/.metronom/release.jks` (folder `700`, file `600`), and the script prints its SHA-256 fingerprint,
which is public. **Back the file up straight away** (two places that are not this Mac) and keep the
password in a password manager: an app signed with a lost key cannot be updated, only uninstalled and
reinstalled, and Google Play cannot recover it either.

Then build, check and sign a release in one command:

```bash
scripts/build-release.sh              # runs scripts/test-all.sh, asks for the key's password, then
                                      # writes dist/Metronom-<version>.apk and its .sha256
scripts/build-release.sh --bundle     # also dist/Metronom-<version>.aab (what Google Play wants)
scripts/build-release.sh --skip-checks   # skip the few-minute test run
```

The script verifies the signature, prints the signer's fingerprint, fails if the APK asks for the
INTERNET permission, and tells you how to install it: `adb install -r dist/Metronom-<version>.apk`.
`dist/` is git-ignored. (By hand, the build reads `METRONOM_KEYSTORE`, `METRONOM_KEYSTORE_PASSWORD`
and `METRONOM_KEY_ALIAS` from the environment.)

**Debug and release can live on one phone.** The debug build has its own application id
(`io.github.martinonstad.metronom.debug`) and is labelled "Metronom (debug)", so installing a release
build does not replace, or wipe, the debug build, and they keep separate libraries. Use
`adb shell run-as io.github.martinonstad.metronom.debug …` for the debug build's files.

**Publishing.** On GitHub, create a release for the tag (for example `v0.9.0`) and attach the APK and
its `.sha256`; put the key's fingerprint in the release notes so people can check that updates come
from you. An update installs over the previous release only if it is signed with the same key and has
a higher `versionCode` (it is derived from the version, so it always grows).

The R8 rules are in `android/app/proguard-rules.pro`. The Rust library is reached through JNA and
the Kotlin bindings UniFFI generates, which JNA finds by reflection, so those classes are kept.
After changing them, **install and run a signed release build on a phone**: a shrinking mistake
only shows up as a crash at run time, not as a build error.

## License notices

The About screen shows `android/app/src/main/assets/licenses.txt`. When a dependency is added,
removed or updated, run `scripts/generate-licenses.sh` and commit the result; `scripts/test-all.sh`
fails when the file is out of date. It reads the license files of the Rust crates (from the cargo
registry), the Android libraries' POMs (from Gradle's cache, so build once first) and
`scripts/licenses/MPL-2.0.txt` (UniFFI's license, copied verbatim from its repository).

## Project conventions

- **`metronom-core` has `#![forbid(unsafe_code)]`.** Only `metronom-ffi`'s audio module may use
  `unsafe`, and every use needs a `// SAFETY:` comment.
- **Real-time code** (the audio callback and anything it calls) must not allocate, lock, or do
  I/O. Communicate with it through atomics.
- **Clamp, don't reject.** Out-of-range user input is clamped to the valid range.
- **Formatting and lints are enforced:** `cargo fmt` and `cargo clippy -D warnings` run in
  `scripts/test-all.sh`.
- **Application id and version.** The id is `io.github.martinonstad.metronom` (the Kotlin package is
  still `no.onstad.metronom`, which does not matter to the phone). It is **permanent** once a release is
  installed anywhere: a different id is a different app, with its own data. The version is the one line
  `val appVersionName` in `android/app/build.gradle.kts`; the `versionCode` is derived from it
  (0.9.0 → 900, 1.0.0 → 10000) so it only ever grows.

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
