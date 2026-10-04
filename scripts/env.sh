#!/usr/bin/env bash
# Source this file (". scripts/env.sh") to put the project's toolchains on PATH.
# Homebrew's rustup is keg-only, so it is not on PATH by default.

for dir in /opt/homebrew/opt/rustup/bin "$HOME/.cargo/bin" /opt/homebrew/opt/openjdk@21/bin; do
  [ -d "$dir" ] && case ":$PATH:" in *":$dir:"*) ;; *) PATH="$dir:$PATH" ;; esac
done
export PATH

if [ -z "${JAVA_HOME:-}" ] && [ -d /opt/homebrew/opt/openjdk@21 ]; then
  export JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
fi

if [ -z "${ANDROID_HOME:-}" ]; then
  for sdk in "$HOME/Library/Android/sdk" /opt/homebrew/share/android-commandlinetools; do
    [ -d "$sdk" ] && export ANDROID_HOME="$sdk" && break
  done
fi
if [ -n "${ANDROID_HOME:-}" ]; then
  export ANDROID_SDK_ROOT="$ANDROID_HOME"
  [ -d "$ANDROID_HOME/platform-tools" ] && PATH="$ANDROID_HOME/platform-tools:$PATH"
  [ -d "$ANDROID_HOME/cmdline-tools/latest/bin" ] && PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
  export PATH
fi
