# Source this from the repo root: `. packaging/android/env.sh`
# Points cargo at the Android NDK. Uses the toolchain under .local/tooling when present,
# otherwise ANDROID_NDK_HOME / ANDROID_HOME from the environment (CI).
_repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
_tool="$_repo/.local/tooling"
if [ -d "$_tool/android/sdk" ]; then
  export ANDROID_HOME="$_tool/android/sdk"
  export ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* | tail -1)"
  export JAVA_HOME="$(ls -d "$_tool"/android/jdk/* | tail -1)"
  export RUSTUP_HOME="$_tool/rustup" CARGO_HOME="$_tool/cargo"
  export PATH="$_tool/cargo/bin:$JAVA_HOME/bin:$PATH"
fi
export ANDROID_API="${ANDROID_API:-29}"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) _host=windows-x86_64; _ext=.cmd ;;
  Darwin) _host=darwin-x86_64; _ext= ;;
  *) _host=linux-x86_64; _ext= ;;
esac
_bin="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$_host/bin"
export CC_aarch64_linux_android="$_bin/aarch64-linux-android$ANDROID_API-clang$_ext"
export CXX_aarch64_linux_android="$_bin/aarch64-linux-android$ANDROID_API-clang++$_ext"
export AR_aarch64_linux_android="$_bin/llvm-ar${_ext:+.exe}"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="$AR_aarch64_linux_android"
export ANDROID_BUILD_TOOLS="$(ls -d "$ANDROID_HOME"/build-tools/* | tail -1)"
export ANDROID_PLATFORM_JAR="$(ls -d "$ANDROID_HOME"/platforms/android-* | tail -1)/android.jar"
# Opus (voice chat) builds with CMake: the NDK's toolchain file and the SDK's Ninja.
export CMAKE_TOOLCHAIN_FILE_aarch64_linux_android="$_repo/packaging/android/arm64.toolchain.cmake"
export CMAKE_GENERATOR_aarch64_linux_android=Ninja
export ANDROID_ABI=arm64-v8a ANDROID_PLATFORM="android-$ANDROID_API"
_cmake="$(ls -d "$ANDROID_HOME"/cmake/* 2>/dev/null | tail -1)"
[ -n "$_cmake" ] && export PATH="$_cmake/bin:$PATH"
