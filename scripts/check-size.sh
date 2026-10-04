#!/usr/bin/env bash
# Builds the release APK (R8-shrunk) and fails if it is 10 MB or larger.
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
