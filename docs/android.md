# Wisp for Android

The phone app runs the same Rust game as the PC (Minecraft: Java Edition 26.3), as a library in a
NativeActivity, no Gradle. The only Java is `WispActivity` (`packaging/android/java`), a
NativeActivity that hides the system bars; `build-apk.sh` compiles it with javac and d8.

| Part | Where |
|---|---|
| Entry point (`android_main`): folders, command line, logcat, opening links (JNI) | `crates/wisp-android` |
| The setup screen, the phone's launcher: sign-in, game files, Play | `crates/wisp-client/src/phone/` (logic, texts, Microsoft sign-in), `game/phone_setup.rs` (window) |
| Touch controls | `crates/wisp-client/src/game/touch.rs` |
| Copying the game files from the PC | `crates/wisp-phone-link` (protocol), `launcher/src/phone_link.rs` + Settings › Phone |
| Manifest, icons, APK build | `packaging/android/` |
| CI and releases (`android-v<version>`) | `.github/workflows/android.yml` |

## Why the files come from the PC

The game reads prepared files (`resources/26.3`, `wisp-resources.json`) that `wisp-prepare` makes
by running Mojang's own Java code (the data generator, registries, exporters) on the player's
Minecraft jars. Android has no JVM to run that, and the files are Mojang's, so they can't ship in
the APK. The launcher on the player's PC offers its prepared folder while Settings › Phone is open:

1. The phone broadcasts `WISP-PHONE-LINK/1 ?` on UDP 25591; launchers answer with their TCP port
   (25592 or the next free one) and the PC's name. Without broadcast the player types the address.
2. `GET /v1/hello` gives the Minecraft version and the prepared layout (`schema`, kept equal to
   `wisp_prepare::SCHEMA` by a test). Another version or layout is refused before copying.
3. `GET /v1/bundle` with the six-digit code in `X-Wisp-Code` streams every file (logs excepted)
   with its SHA-256 (`bundle.rs`). Paths are checked before anything is written. Files land in
   `26.3.part` and replace `26.3` only when complete. Five wrong codes close the offer; it ends
   after 15 idle minutes.

## On the phone

- Home (the game folder, worlds, logs, resources): `Android/data/dev.wisp.client/files`,
  reachable from a PC over USB. The sign-in (Microsoft refresh token) stays in the app's private
  folder. The app answers the game's session renewals itself (`phone::msa::renew_in_background`).
- Android allows one winit event loop per process: the setup screen and the game share it
  (`game::run_app`, `run_app_on_demand`), and the game's `resumed` is replayed for the second app.
- The system bars are hidden "sticky" (`WispActivity`): a swipe from an edge first only shows
  them, so a thumb sliding off the screen doesn't go home. The touch buttons keep
  `Layout::edge` (0.45 units) free along the edges, and back on the setup's home does nothing.
- Phone options (`phone::apply_phone_defaults`, once per `PHONE_DEFAULTS_VERSION`): fast
  graphics, render distance 6 and simulation distance 5, 60 FPS, no clouds/shadows/smooth
  lighting, the world at 50 % resolution, a 2 MiB chunk upload budget per frame, culling and
  battery mode. Bump the version to give installs new ones.
- Going to the background drops the window surface (`Renderer::suspend_surface`), pauses and saves
  the world; coming back makes a new surface.
- mimalloc is built with local-dynamic TLS on Android (a dlopened library can't use initial-exec).
- The library is linked for 16 KB pages (`crates/wisp-android/build.rs`), which Android 15 phones need.
- Not yet: Bluetooth controllers (gilrs/hidapi are off), the system clipboard, asking for the
  microphone permission (voice chat works once it's allowed in the app settings).

## Build

```sh
# Once: Rust target, Android SDK (build-tools 35, platform 35, NDK 27, CMake), JDK 17.
rustup target add aarch64-linux-android
# Local tooling under .local/tooling/android is found by env.sh; else ANDROID_HOME/ANDROID_NDK_HOME.
packaging/android/build-apk.sh --profile fast      # quick, debug-signed (.local/android-debug.jks)
WISP_ANDROID_KEYSTORE=... WISP_ANDROID_KEYSTORE_PASS=... WISP_ANDROID_KEY_ALIAS=wisp \
  packaging/android/build-apk.sh --profile release # the release APK in dist/android
adb install -r dist/android/*.apk && adb logcat -s wisp
```

The setup screen also runs on a PC: `wisp-client --phone-private <dir> --resources <dir>`.

The release key lives outside the repository (the owner's backup) and in the repository secrets
`WISP_ANDROID_KEYSTORE_BASE64`, `WISP_ANDROID_KEYSTORE_PASS` and `WISP_ANDROID_KEY_ALIAS`. Every
update must be signed with the same key, or phones refuse to install it over the old app.
