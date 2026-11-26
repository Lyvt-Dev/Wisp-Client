#!/usr/bin/env bash
# Packages Wisp Client for Linux as an AppImage: one file to download, make executable and run.
# It holds the launcher, the Rust game engine (wisp-client, wisp-prepare and the preparation kit,
# as the Windows setup does) and the libraries they need beside them. The C library, the graphics drivers and the session
# libraries stay the machine's (see `system_library`): a Vulkan driver or a glibc from another
# machine cannot be packaged.
#
#   packaging/linux/appimage.sh BINARY VERSION OUTDIR [ENGINE_DIR]
#
# ENGINE_DIR holds wisp-client, wisp-prepare and kit/ (research-lock.json, LICENSE and the
# exporter sources). Without it the image has the launcher alone, which can't install the game.
#
# Needs patchelf, and appimagetool: the AppImage project's, 1.9.1 by default, downloaded here and checked against
# its SHA-256 ($APPIMAGETOOL names one already on the machine, as the .dmg build takes $DMGBUILD).
# It runs without FUSE (`--appimage-extract-and-run`), so a runner needs none.
#
# The image is mounted at a new temporary path on every start, while an instance keeps the full
# path of its game. So AppRun copies the engine once per engine build to the launcher's data folder
# (engine/) and points the launcher there with WISP_ENGINE_DIR.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
binary=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
version=$2
out=$(mkdir -p "$3" && cd "$3" && pwd)
engine=${4:+$(cd "$4" && pwd)}
name=Wisp-Client-$version-x86_64.AppImage

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
appdir=$work/AppDir
mkdir -p "$appdir/usr/bin" "$appdir/usr/lib" \
  "$appdir/usr/share/applications" "$appdir/usr/share/icons"

# The program.
cp "$binary" "$appdir/usr/bin/wisp-launcher"
chmod 755 "$appdir/usr/bin/wisp-launcher"

# The engine, beside the launcher where `engine::bundled` looks (usr/bin/engine), with a stamp
# naming this build so AppRun knows when its copy outside the image is out of date.
if [[ -n "$engine" ]]; then
  for f in wisp-client wisp-prepare kit/research-lock.json; do
    [[ -e "$engine/$f" ]] || { echo "error: $engine/$f is missing" >&2; exit 1; }
  done
  cp -R "$engine" "$appdir/usr/bin/engine"
  chmod 755 "$appdir/usr/bin/engine/wisp-client" "$appdir/usr/bin/engine/wisp-prepare"
  (cd "$appdir/usr/bin/engine" && find . -type f ! -name .stamp -print0 | sort -z     | xargs -0 sha256sum | sha256sum | cut -d' ' -f1) > "$appdir/usr/bin/engine/.stamp"
fi

# Both places: AppRun's, and the one desktop environments read once the image is unpacked.
cp "$here/wisp-launcher.desktop" "$appdir/wisp-launcher.desktop"
cp "$here/wisp-launcher.desktop" "$appdir/usr/share/applications/wisp-launcher.desktop"
cp -R "$here/../icons/hicolor/." "$appdir/usr/share/icons/hicolor/"
cp "$here/../icons/hicolor/512x512/apps/wisp.png" "$appdir/wisp.png"
ln -s wisp.png "$appdir/.DirIcon"

# Runs the packaged binary. It finds the packaged libraries through its own run path (set below),
# not LD_LIBRARY_PATH, which every program it starts (the game, a browser, a file manager) would
# inherit and load the launcher's libraries with.
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
# AppRun: the launcher; its libraries are beside it (usr/lib, through its run path).
here=$(dirname "$(readlink -f "$0")")
# The engine is copied out of the image, which is mounted somewhere new each start, so the game
# path an instance keeps stays valid. Copied whole and swapped in by renames: a game still running
# from the old copy keeps its files.
if [ -f "$here/usr/bin/engine/.stamp" ] && [ -z "${WISP_ENGINE_DIR:-}" ]; then
  data=${WISP_LAUNCHER_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/wisp-local-launcher}
  dest=$data/engine
  if ! cmp -s "$here/usr/bin/engine/.stamp" "$dest/.stamp" 2>/dev/null; then
    mkdir -p "$data" && rm -rf "$dest.new" "$dest.old"       && cp -R "$here/usr/bin/engine" "$dest.new"       && { [ ! -e "$dest" ] || mv "$dest" "$dest.old"; }       && mv "$dest.new" "$dest" && rm -rf "$dest.old"
  fi
  if cmp -s "$here/usr/bin/engine/.stamp" "$dest/.stamp" 2>/dev/null; then
    WISP_ENGINE_DIR=$dest
    export WISP_ENGINE_DIR
  fi
fi
exec "$here/usr/bin/wisp-launcher" "$@"
EOF
chmod 755 "$appdir/AppRun"

# Libraries that must come from the machine: bundling any of them breaks it, in one of two ways.
system_library() {
  case "$(basename "$1")" in
    # The C library: the loader has to be the system's, and two copies of glibc in one process
    # do not work. Every library here links it, so the AppImage runs where glibc is at least the
    # build machine's (ubuntu-22.04: 2.35, so Debian 12, Ubuntu 22.04, Fedora 36 and newer).
    libc.so.* | libm.so.* | libpthread.so.* | libdl.so.* | librt.so.* | libresolv.so.* | \
      libnss_*.so.* | libutil.so.* | libanl.so.* | ld-linux*.so.* | libgcc_s.so.*) return 0 ;;
    # The graphics stack: the Vulkan loader, the GL libraries, Mesa's and the vendors' drivers
    # are matched to the GPU and its driver package on this machine.
    libGL.so.* | libGLX.so.* | libEGL.so.* | libOpenGL.so.* | libGLdispatch.so.* | \
      libglapi.so.* | libvulkan.so.* | libdrm.so.* | libgbm.so.* | libva.so.* | \
      libvdpau.so.*) return 0 ;;
    # The session's: D-Bus talks to the running session's bus (the keychain is a service on it),
    # and systemd/udev are the machine's device and service managers.
    libdbus-1.so.* | libsystemd.so.* | libudev.so.*) return 0 ;;
    # Sound: ALSA's plugins are the machine's and are loaded by name.
    libasound.so.*) return 0 ;;
  esac
  return 1
}

# Everything else the launcher and the engine link, then everything those need, until nothing new
# is added.
todo=("$appdir/usr/bin/wisp-launcher")
if [[ -n "$engine" ]]; then
  todo+=("$appdir/usr/bin/engine/wisp-client" "$appdir/usr/bin/engine/wisp-prepare")
fi
seen=" "
while [[ ${#todo[@]} -gt 0 ]]; do
  elf=${todo[0]}
  todo=("${todo[@]:1}")
  while read -r soname arrow path _; do
    [[ "$arrow" == "=>" && -f "$path" ]] || continue
    base=$(basename "$path")
    system_library "$path" && continue
    [[ "$seen" == *" $base "* ]] && continue
    seen+="$base "
    cp -L "$path" "$appdir/usr/lib/$base"
    todo+=("$appdir/usr/lib/$base")
  done < <(ldd "$elf")
done

# Each packaged file looks for the others beside the launcher: the binary in ../lib, a library in
# its own folder. A run path is the file's own, so nothing the launcher starts inherits it.
command -v patchelf > /dev/null || { echo "error: patchelf is needed (apt install patchelf)" >&2; exit 1; }
patchelf --set-rpath '$ORIGIN/../lib' "$appdir/usr/bin/wisp-launcher"
# The engine runs from its copy outside the image too (AppRun), so it takes its libraries along.
if [[ -n "$engine" ]]; then
  shopt -s nullglob
  libs=("$appdir"/usr/lib/*.so*)
  if [[ ${#libs[@]} -gt 0 ]]; then
    mkdir -p "$appdir/usr/bin/engine/lib"
    cp -L "${libs[@]}" "$appdir/usr/bin/engine/lib/"
    for lib in "$appdir"/usr/bin/engine/lib/*.so*; do patchelf --set-rpath '$ORIGIN' "$lib"; done
  fi
  shopt -u nullglob
  patchelf --set-rpath '$ORIGIN/lib' "$appdir/usr/bin/engine/wisp-client" "$appdir/usr/bin/engine/wisp-prepare"
fi
for lib in "$appdir"/usr/lib/*.so*; do
  if [[ -e "$lib" ]]; then patchelf --set-rpath '$ORIGIN' "$lib"; fi
done

# Everything the launcher links is either beside it or the machine's: a library missing on the
# machine that builds this would make an image that starts nowhere. No LD_LIBRARY_PATH: the run
# paths alone must find them.
missing=$(for elf in "$appdir/usr/bin/wisp-launcher" "$appdir"/usr/bin/engine/wisp-*; do
  [[ -f "$elf" ]] && ldd "$elf"
done | grep "not found" || true)
[[ -z "$missing" ]] || {
  echo "error: the launcher links libraries this machine does not have:" >&2
  echo "$missing" >&2
  exit 1
}

tool=${APPIMAGETOOL:-}
if [[ -z "$tool" ]]; then
  tool=$work/appimagetool
  # Pinned: the same tool for every build, whatever the AppImage project publishes next. 1.9.1's
  # runtime mounts the image where FUSE 2 is there and unpacks it where it is not.
  url=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
  sha256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
  curl -fsSL -o "$tool" "$url"
  chmod 755 "$tool"
  echo "$sha256  $tool" | sha256sum -c - > /dev/null
fi

rm -f "$out/$name"
# Unpacked first, so this works on a machine (or runner) without FUSE.
ARCH=x86_64 "$tool" --appimage-extract-and-run --no-appstream "$appdir" "$out/$name"

# It starts, and is the version it was built as, before any window or GPU is needed.
got=$("$out/$name" --appimage-extract-and-run --version)
[[ "$got" == "wisp-launcher $version" ]] || {
  echo "error: expected 'wisp-launcher $version', got '$got'" >&2
  exit 1
}

# AppRun copies the engine out and the launcher finds it there. A data folder of its own: never a
# player's.
if [[ -n "$engine" ]]; then
  WISP_LAUNCHER_HOME=$work/home "$out/$name" --appimage-extract-and-run --version > /dev/null
  [[ -x "$work/home/engine/wisp-client" && -f "$work/home/engine/kit/research-lock.json" ]] || {
    echo "error: AppRun did not copy the engine to the launcher's data folder" >&2
    exit 1
  }
  if ldd "$work/home/engine/wisp-client" | grep -q "not found"; then
    echo "error: the copied engine is missing libraries" >&2
    exit 1
  fi
fi

file "$out/$name"
echo "Packaged $out/$name"
