# Build/run environment for the Wisp game workspace on NixOS.
#   nix-shell            # from the repo root, then:
#   cargo build --profile fast -p wisp-client --bin wisp-client
#
# Linux is a supported build target (see .github/workflows/game-build.yml, which
# builds on ubuntu-22.04). The native libraries below mirror that CI plus
# launcher/shell.nix; Python 3 and JDK 25 are for `wisp-prepare`, which turns
# YOUR OWN copy of Minecraft: Java Edition 26.3 into the game's resources.
{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    # Rust toolchain (nixpkgs ships 1.95; the workspace needs 1.90+).
    rustc
    cargo
    clippy
    rustfmt
    # Build helpers.
    pkg-config
    clang
    mold
    cmake          # libopus via audiopus_sys ships a pre-3.5 CMakeLists.
    # wisp-prepare.
    python3
    jdk25
  ];

  buildInputs = with pkgs; [
    # Windowing / input (winit).
    libxkbcommon
    wayland
    libx11
    libxcb
    libxcursor
    libxrandr
    libxi
    # GPU (wgpu / Vulkan).
    vulkan-loader
    # Text.
    fontconfig
    freetype
    # Audio + device enumeration.
    alsa-lib
    udev
    # TLS.
    openssl
  ];

  # libopus' old CMakeLists needs a CMake policy floor (matches .cargo/config.toml).
  CMAKE_POLICY_VERSION_MINIMUM = "3.5";

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  # Vulkan/X11/Wayland/audio are dlopen'd at runtime, so they must be on the
  # loader path when the game runs from inside this shell.
  LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [
    libxkbcommon wayland vulkan-loader fontconfig freetype alsa-lib udev
    libx11 libxcb libxcursor libxrandr libxi
  ];
}
