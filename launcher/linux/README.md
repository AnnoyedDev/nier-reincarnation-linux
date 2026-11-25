# NieR Re[in]carnation - Linux launcher

Version 1.0.0, developed by **AnnoyedDev**. [Version française](README.fr.md)

A native Linux launcher for the NieR Re[in]carnation PC port. The window, the server and the setup
all run natively. Only the game's runtime goes through Wine or Proton.

## Credits

This launcher is a Linux adaptation of
**[AllenWalkerED/nier-reincarnation-pc](https://github.com/AllenWalkerED/nier-reincarnation-pc)**.
Its pages, its settings, its texts, its setup and the whole PC port come from that project.
None of this would exist without it.

The original port itself stands on:

- [Lunar Tear](https://github.com/Walter-Sparrow/lunar-tear), the server that brought the game back
- [win.apk](https://github.com/israel2176/win.apk), the Android runtime
- [Bunker](https://github.com/veritr1x/bunker), its APK patches and its server changes
- [Lunar Base](https://github.com/NMeliksah/lunar-base), the
  [Lunar Tear Comfort Mod](https://github.com/NavHobbyDev/lunar-tear-comfort-mod) and
  [lunar-tear-masterdata-patcher](https://github.com/NavHobbyDev/lunar-tear-masterdata-patcher)
- ANGLE, Eclipse Temurin, LLVM, texture2ddecoder, UnityPy and 7-Zip

Each one's details and license are in the original project's `NOTICE.md` and `licenses/` folder.

## What this version changes

- the launcher's menus in English or French (Settings > Language > Launcher)
- no update check: the launcher contacts no website
- your own environment variables for the game (Settings > System), for example `MANGOHUD=1`
- a launcher written in Rust (egui) in place of the Windows WebView2 page

## Building

Linux only, with stable Rust and `cargo`:

```sh
cargo build --release   # gives target/release/nier-launcher
cargo test
```

The binary goes at the root of the game folder, which the original project's `scripts/build_linux.sh`
assembles. To try a build without installing it:

```sh
NIER_ROOT=<game folder> target/release/nier-launcher
```

## License

AGPL-3.0-only, like the original project.

This is an unofficial, non-commercial fan project, not affiliated with SQUARE ENIX. No game file is
included: you need your own copy of NieR Re[in]carnation.
