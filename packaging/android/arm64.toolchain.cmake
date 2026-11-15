# CMake-built C dependencies (Opus) for arm64 phones: the NDK's own toolchain with its ABI set,
# since the cmake crate passes neither ANDROID_ABI nor ANDROID_PLATFORM.
set(ANDROID_ABI arm64-v8a)
if(NOT ANDROID_PLATFORM)
  set(ANDROID_PLATFORM android-29)
endif()
include("$ENV{ANDROID_NDK_HOME}/build/cmake/android.toolchain.cmake")
