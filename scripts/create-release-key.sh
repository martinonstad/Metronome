#!/usr/bin/env bash
# Creates the key that signs your release builds. Run it ONCE, yourself, in a terminal: keytool
# asks you for a password (nothing is shown as you type) and nobody else ever sees it.
#
#   scripts/create-release-key.sh
#
# The key lands in ~/.metronom/release.jks (folder 700, file 600). It is never in the repository
# (*.jks is git-ignored). BACK IT UP the moment it exists, and keep the password in a password
# manager: an app signed with a lost key cannot be updated, only uninstalled and reinstalled.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/env.sh
. scripts/env.sh

dir="${METRONOM_KEY_DIR:-$HOME/.metronom}"
store="$dir/release.jks"
alias_name="${METRONOM_KEY_ALIAS:-metronom}"

if [ -e "$store" ]; then
  echo "A key already exists at $store. Not touching it." >&2
  echo "(To make a different one, set METRONOM_KEY_DIR to another folder.)" >&2
  exit 1
fi

mkdir -p "$dir"
chmod 700 "$dir"

echo "Creating the release key in $store"
echo "You will be asked for a password twice. Choose a long one and put it in your password manager."
echo
keytool -genkeypair -keystore "$store" -storetype PKCS12 -alias "$alias_name" \
  -keyalg RSA -keysize 4096 -validity 10000 \
  -dname "${METRONOM_KEY_DNAME:-CN=Martin Onstad}"
chmod 600 "$store"

echo
echo "Key created. Enter the password once more to show its fingerprint:"
keytool -list -keystore "$store" -alias "$alias_name" | grep -E "Certificate fingerprint|SHA-?256" || true

cat <<EOM

Now:
  1. Copy $store to at least two places that are not this Mac (a USB stick, a cloud drive you trust).
  2. Save the password in your password manager.
  3. Build a signed release with: scripts/build-release.sh
The fingerprint above is public; you can publish it so people can check that updates come from you.
EOM
