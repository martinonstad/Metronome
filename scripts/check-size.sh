#!/usr/bin/env bash
# Builds the release APK (R8-shrunk) and fails if it is 10 MB or larger or asks for the
# INTERNET permission (the app is offline by design: see docs/privacy.md).
# Without signing variables the APK is unsigned: fine for measuring, not installable
# (see docs/development.md for signing).
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/env.sh
. scripts/env.sh

limit=$((10 * 1024 * 1024))
(cd android && ./gradlew --console=plain --quiet assembleRelease)
apk=$(ls android/app/build/outputs/apk/release/*.apk | head -1)
size=$(stat -f%z "$apk" 2>/dev/null || stat -c%s "$apk")
echo "release APK: $(basename "$apk"), $size bytes (limit $limit)"
if [ "$size" -ge "$limit" ]; then
  echo "The release APK is too large." >&2
  exit 1
fi

aapt2="$ANDROID_HOME/build-tools/$(ls "$ANDROID_HOME/build-tools" | sort -V | tail -1)/aapt2"
permissions=$("$aapt2" dump permissions "$apk")
if echo "$permissions" | grep -q "android.permission.INTERNET"; then
  echo "The release APK asks for the INTERNET permission; the app must stay offline." >&2
  exit 1
fi
echo "permissions: $(echo "$permissions" | grep -o "name='[^']*'" | grep -v DYNAMIC_RECEIVER | sed "s/name='android.permission.//; s/'//" | paste -sd, -)"
