#!/usr/bin/env bash
# Builds a signed release APK (and optionally an App Bundle) with your release key, checks it, and
# puts it in dist/ with its SHA-256. It asks for the key's password without showing it.
#
#   scripts/build-release.sh              run all checks, then build dist/Metronom-<version>.apk
#   scripts/build-release.sh --bundle     also build the App Bundle (.aab, for Google Play)
#   scripts/build-release.sh --skip-checks   skip scripts/test-all.sh (it takes a few minutes)
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/env.sh
. scripts/env.sh

bundle=0 checks=1
for arg in "$@"; do
  case "$arg" in
    --bundle) bundle=1 ;;
    --skip-checks) checks=0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

store="${METRONOM_KEYSTORE:-$HOME/.metronom/release.jks}"
alias_name="${METRONOM_KEY_ALIAS:-metronom}"
if [ ! -f "$store" ]; then
  echo "No release key at $store. Create it first: scripts/create-release-key.sh" >&2
  exit 1
fi

version=$(sed -n 's/^val appVersionName = "\(.*\)".*/\1/p' android/app/build.gradle.kts | head -1)
[ -n "$version" ] || { echo "Could not read appVersionName from android/app/build.gradle.kts" >&2; exit 1; }
echo "Building Metronom $version"

if [ "$checks" = 1 ]; then
  scripts/test-all.sh
fi

read -r -s -p "Password of the release key: " METRONOM_KEYSTORE_PASSWORD
echo
export METRONOM_KEYSTORE="$store" METRONOM_KEYSTORE_PASSWORD METRONOM_KEY_ALIAS="$alias_name"

# Fail early, and clearly, on a wrong password.
if ! keytool -list -keystore "$store" -alias "$alias_name" -storepass:env METRONOM_KEYSTORE_PASSWORD >/dev/null 2>&1; then
  echo "That password does not open $store (or the alias '$alias_name' is missing)." >&2
  exit 1
fi

tasks=(assembleRelease)
[ "$bundle" = 1 ] && tasks+=(bundleRelease)
(cd android && ./gradlew --console=plain --quiet "${tasks[@]}")

apk=android/app/build/outputs/apk/release/app-release.apk
build_tools="$ANDROID_HOME/build-tools/$(ls "$ANDROID_HOME/build-tools" | sort -V | tail -1)"
echo "== Signature =="
"$build_tools/apksigner" verify --print-certs "$apk" | grep -E "Signer #1 certificate (DN|SHA-256)"
permissions=$("$build_tools/aapt2" dump permissions "$apk")
if echo "$permissions" | grep -q "android.permission.INTERNET"; then
  echo "The APK asks for the INTERNET permission; the app must stay offline." >&2
  exit 1
fi

mkdir -p dist
cp "$apk" "dist/Metronom-$version.apk"
(cd dist && shasum -a 256 "Metronom-$version.apk" > "Metronom-$version.apk.sha256" && cat "Metronom-$version.apk.sha256")
if [ "$bundle" = 1 ]; then
  cp android/app/build/outputs/bundle/release/app-release.aab "dist/Metronom-$version.aab"
fi
ls -la dist | sed 's/^/  /'
echo
echo "Install on a connected phone:  adb install -r dist/Metronom-$version.apk"
echo "(A release build is signed differently from a debug build. If the debug build has the same"
echo " application id, uninstall it first; that deletes its data, so export your library before.)"
