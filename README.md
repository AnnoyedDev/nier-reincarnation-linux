# The fork

Added a complete rewrite in rust and some addition to make it work flawlessly on Linux, [more info here](launcher/linux/)

# NieR Re[in]carnation — PC port (unofficial)


NieR Re[in]carnation died on April 29, 2024. This brings it back, running natively on Windows: your
own copy, no emulator, no internet, no phone.

Fan project, not affiliated with SQUARE ENIX or Applibot, and **the game isn't in here**. You bring
your own files and the launcher turns them into a PC install. (Legal stuff: [NOTICE.md](NOTICE.md).)

![The launcher](docs/screenshots/settings-display.webp)

## So what kind of port is this?

Not an emulator. There's no virtual phone, no Android running in the background. And not a source port
either, since nobody has the game's source.

It's the actual Android game, running natively as a Windows program. Its arm64 code gets translated to
native x86-64 code (on the fly at first, then ahead of time in the background, so it gets faster as you play), the bits of
Android it expects are faked by the runtime, kinda like Wine does for Windows apps on Linux, and the
graphics go through ANGLE to Direct3D 11 or Vulkan. The game itself is basically untouched, just a few
small patches like the server address and the 30 fps cap.

The official servers are gone, so the port runs [Lunar Tear](https://github.com/Walter-Sparrow/lunar-tear)
locally, the fan-made server that brought the game back. Nothing leaves your PC.

## Download

Grab `NieR-Reincarnation-PC-v1.2.zip` from [Releases](../../releases). Already playing an older version?
Unzip it over your game folder, your saves and settings stay.

Got a save on your phone (Bunker)? Export it there, then Settings › Saves › **Import from phone**.

## What you need

* Windows 10 or 11 (64-bit)
* A GPU that does Direct3D 11 or Vulkan
* ~25 GB free, 8 GB RAM recommended
* WebView2 (Windows 11 already has it)

Plus three game files:

1. the arm64 APK, version **3.7.1-152** (`com.square_enix.android_googleplay.nierspww`)
2. `resource_dump_android.7z` (the asset dump)
3. `20240404193219.bin.e` (the master data)

The asset dump and master data are on the [Lunar Tear](https://github.com/Walter-Sparrow/lunar-tear)
Discord: https://discord.gg/MZAf5aVkJG. The APK? Google it, it's out there.

Don't post any of these files in issues, please.

## Setup

Unzip it anywhere, run `NieR.exe`, feed it the three files. First run unpacks ~21 GB and patches
everything, so go grab a coffee. You only do this once, and your original files aren't touched.

Hit **START**. The first session or two will feel a bit sluggish while the game's code gets translated
in the background (~15 min). After that you're good.

## The launcher

Made to look like the game's own menus. You get the game's quality presets plus a bunch of extras
(upscaling, FXAA, frame rates up to unlimited), language and voice, some optional mods for the local
server, and keyboard/controller bindings.

![Graphics](docs/screenshots/settings-graphics.webp)

![Performance](docs/screenshots/settings-performance.webp)

The **Events** tab lets you pick which banners and events are live. Everything's closed until you open
it, except the permanent summons.

![Events](docs/screenshots/events.webp)

And there's a save manager: multiple saves, plus an editor for what your save owns, with backups before
every change.

![Saves](docs/screenshots/saves.webp)

![Save editor](docs/screenshots/save-editor.webp)

## Tested on

RTX 3060, 3080, 4060 and 5060, an RX 580, and Intel integrated graphics (Iris Xe and others). Several
people have put long sessions into it.

## Something broke?

* **Black 3D or weird graphics:** switch the renderer in Settings › Display (Direct3D 11 is the default,
  try Vulkan).
* **Slow at first:** that's the background translation, give it a session or two.
* **SmartScreen / antivirus yelling at you:** the exes aren't signed. Everything's built from the source
  right here.
* **Bug report:** Settings › System › **Export logs**, then open an issue and attach the zip. It leaves
  out your Windows username and PC name.

Known quirks: a few controller buttons do double duty on different screens. A Mac version is in the works.

## Linux

Same port, same launcher page, Linux-native everywhere except the game's runtime:

* **native:** the launcher (`nier-launcher`, Rust, the same menus drawn natively), the server (a Linux build of
  `nier-server`), the setup and patchers (your Python 3, in `tools/venv`, made at the first setup)
* **in Wine/Proton:** only the win.apk runtime (the game), in its own prefix (`prefix/` in the game
  folder). Auto picks umu-run, then Proton (GE-Proton first), then Wine; change it in Settings ›
  System › Compatibility layer. Vulkan is the default renderer (Direct3D 11 goes through DXVK).

You need Python 3, 7-Zip (`7z`), and Steam's Proton, umu-launcher or Wine. Run
`./nier-launcher` and feed it the same three files.

## Building

* **Runtime:** win.apk at `640f38f06c4934664767abc365a43b3c6bd7dea3` + `git apply winapk/winapk-nier.patch`,
  then build per its README (MSYS2 CLANG64, CMake)
* **Server:** `cd server && go build ./cmd/...` (Go 1.22+)
* **Launcher:** `cd launcher && OUT=<folder> bash build.sh` (MSYS2 CLANG64, WebView2 SDK)
* **Release folder:** `python scripts/build_dist.py --out <folder> ...`
* **Linux release folder:** `scripts/build_linux.sh --out <folder> --runtime <the runtime folder, or an
  extracted Windows release>` (Go and cargo; the launcher is `launcher/linux`)

## Credits

The PC port is my work.

Huge thanks to [Lunar Tear](https://github.com/Walter-Sparrow/lunar-tear) for bringing the game back,
[win.apk](https://github.com/israel2176/win.apk) for the runtime underneath, and
[Bunker](https://github.com/veritr1x/bunker), whose APK patch modules and server changes are included
here under its MIT license. Also [Lunar Base](https://github.com/NMeliksah/lunar-base), the
[Lunar Tear Comfort Mod](https://github.com/NavHobbyDev/lunar-tear-comfort-mod),
[lunar-tear-masterdata-patcher](https://github.com/NavHobbyDev/lunar-tear-masterdata-patcher), ANGLE,
Eclipse Temurin, LLVM, texture2ddecoder, UnityPy and 7-Zip. And everyone who tested it.

## License

AGPL-3.0 ([LICENSE](LICENSE)). Bundled stuff keeps its own license, see [NOTICE.md](NOTICE.md) and
`licenses/`.
