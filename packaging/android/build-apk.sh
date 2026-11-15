#!/usr/bin/env bash
# Builds the Wisp phone app: libwisp_android.so for arm64 phones, packed with the manifest and
# icons into a signed APK. No Gradle: aapt2, zipalign and apksigner from the Android SDK.
#
#   packaging/android/build-apk.sh [--profile release|fast] [--out DIR]
#
# Signing: WISP_ANDROID_KEYSTORE (a .jks file), WISP_ANDROID_KEYSTORE_PASS and
# WISP_ANDROID_KEY_ALIAS sign a release. Without them a local debug key in
# .local/android-debug.jks signs the APK (installable for testing, not for updates of a release).
#
# Needs: the Android SDK (build-tools, a platform, the NDK and CMake), a JDK, and the Rust target
# aarch64-linux-android. packaging/android/env.sh finds them.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
profile=release
out="$repo/dist/android"
while [ $# -gt 0 ]; do
  case "$1" in
    --profile) profile="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done

# shellcheck source=env.sh
. "$here/env.sh"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target/android}"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/launcher/Cargo.toml" | head -1)"
# versionCode: 0.15.2 -> 1502 (major*100000 + minor*100 + patch), always rising with the version.
IFS=. read -r major minor patch <<<"$version"
code=$((major * 100000 + minor * 100 + patch))
echo "Wisp for Android $version (versionCode $code), profile $profile"

cargo build --target aarch64-linux-android -p wisp-android --profile "$profile"
lib="$CARGO_TARGET_DIR/aarch64-linux-android/$profile/libwisp_android.so"

case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) exe=.exe; bat=.bat ;; *) exe=; bat= ;; esac
bt="$ANDROID_BUILD_TOOLS"
work="$CARGO_TARGET_DIR/apk"
rm -rf "$work" && mkdir -p "$work/res" "$work/lib/arm64-v8a" "$out"

# Strip debug info: the library is several times smaller.
"$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$_host/bin/llvm-strip$exe" --strip-debug \
  -o "$work/lib/arm64-v8a/libwisp_android.so" "$lib"

"$bt/aapt2$exe" compile --dir "$here/res" -o "$work/res.zip"
"$bt/aapt2$exe" link -o "$work/unsigned.apk" \
  --manifest "$here/AndroidManifest.xml" \
  -I "$ANDROID_PLATFORM_JAR" \
  --min-sdk-version 29 --target-sdk-version 35 \
  --version-code "$code" --version-name "$version" \
  "$work/res.zip"
py="$(command -v python || command -v python3)"
(cd "$work" && "$py" -c 'import zipfile
with zipfile.ZipFile("unsigned.apk", "a", zipfile.ZIP_DEFLATED) as z:
    z.write("lib/arm64-v8a/libwisp_android.so")')
"$bt/zipalign$exe" -p -f 4 "$work/unsigned.apk" "$work/aligned.apk"

if [ -n "${WISP_ANDROID_KEYSTORE:-}" ]; then
  keystore="$WISP_ANDROID_KEYSTORE"
  pass="${WISP_ANDROID_KEYSTORE_PASS:?WISP_ANDROID_KEYSTORE_PASS is needed with the keystore}"
  alias="${WISP_ANDROID_KEY_ALIAS:-wisp}"
  name="Wisp-Client-$version-android-arm64.apk"
else
  keystore="$repo/.local/android-debug.jks"
  pass=android
  alias=androiddebugkey
  if [ ! -f "$keystore" ]; then
    keytool -genkeypair -keystore "$keystore" -storepass "$pass" -keypass "$pass" \
      -alias "$alias" -keyalg RSA -keysize 2048 -validity 10000 \
      -dname "CN=Wisp Debug" >/dev/null
  fi
  name="Wisp-Client-$version-android-arm64-debug.apk"
fi
"$bt/apksigner$bat" sign --ks "$keystore" --ks-pass "pass:$pass" --key-pass "pass:$pass" \
  --ks-key-alias "$alias" --out "$out/$name" "$work/aligned.apk"
"$bt/apksigner$bat" verify "$out/$name"
(cd "$out" && sha256sum "$name" > "$name.sha256")
echo "$out/$name"
