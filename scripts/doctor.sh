#!/usr/bin/env bash
# Checks that everything needed to build and test Metronom is installed.
# Exit status is non-zero if something needed for Android development is missing.
cd "$(dirname "$0")/.." || exit 1
# shellcheck source=scripts/env.sh
. scripts/env.sh

missing=0
ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad()  { printf '  \033[31m✗\033[0m %s\n      fix: %s\n' "$1" "$2"; missing=1; }
note() { printf '  \033[33m·\033[0m %s\n' "$1"; }

echo "Rust"
if command -v rustc >/dev/null; then ok "$(rustc --version)"; else bad "rustc not found" "brew install rustup && rustup default stable"; fi
if command -v cargo >/dev/null; then
  installed_targets="$(rustup target list --installed 2>/dev/null)"
  for t in aarch64-linux-android x86_64-linux-android; do
    case "$installed_targets" in *"$t"*) ok "target $t" ;; *) bad "target $t missing" "rustup target add $t" ;; esac
  done
  command -v cargo-ndk >/dev/null && ok "cargo-ndk" || bad "cargo-ndk missing" "cargo install cargo-ndk --locked"
fi

echo "Java (Gradle needs 17 or 21; 25 is too new for the Android Gradle Plugin)"
if command -v java >/dev/null; then
  major="$(java -version 2>&1 | sed -n 's/.*version "\([0-9]*\).*/\1/p' | head -1)"
  case "$major" in
    17|21) ok "java $major" ;;
    *) bad "java $major is not 17 or 21" "brew install openjdk@21" ;;
  esac
else
  bad "java not found" "brew install openjdk@21"
fi

echo "Android SDK"
if [ -n "${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME" ]; then
  ok "ANDROID_HOME=$ANDROID_HOME"
  command -v sdkmanager >/dev/null && ok "sdkmanager" || bad "sdkmanager not found" "brew install --cask android-commandlinetools"
  ls "$ANDROID_HOME"/ndk/* >/dev/null 2>&1 && ok "NDK $(ls "$ANDROID_HOME/ndk" | tail -1)" || bad "NDK missing" "sdkmanager 'ndk;<version>'"
  ls "$ANDROID_HOME"/platforms/* >/dev/null 2>&1 && ok "platform $(ls "$ANDROID_HOME/platforms" | tail -1)" || bad "no Android platform installed" "sdkmanager 'platforms;android-<N>'"
  ls "$ANDROID_HOME"/build-tools/* >/dev/null 2>&1 && ok "build-tools $(ls "$ANDROID_HOME/build-tools" | tail -1)" || bad "build-tools missing" "sdkmanager 'build-tools;<version>'"
else
  bad "Android SDK not found" "brew install --cask android-commandlinetools (then sdkmanager ...)"
fi
if command -v adb >/dev/null; then
  ok "adb $(adb version | head -1)"
  devices="$(adb devices | sed 1d | grep -w device || true)"
  if [ -n "$devices" ]; then ok "device connected: $(echo "$devices" | awk '{print $1}' | tr '\n' ' ')"; else note "no phone connected (enable USB debugging, plug in, accept the prompt)"; fi
else
  bad "adb not found" "brew install --cask android-platform-tools"
fi

echo "iPhone (only needed for Milestone 5)"
if xcodebuild -version >/dev/null 2>&1; then ok "$(xcodebuild -version | head -1)"; else note "Xcode not installed (later: Mac App Store)"; fi

exit $missing
