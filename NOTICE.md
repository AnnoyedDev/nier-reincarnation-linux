# Notice

**NieR Re[in]carnation PC port** is an unofficial, non-commercial fan project. It is not made,
endorsed or supported by SQUARE ENIX, Applibot or any of their partners.
*NieR*, *NieR Re[in]carnation* and their logos are trademarks of SQUARE ENIX CO., LTD.

**No part of the game is distributed with this port.** It runs only with files the player supplies
from their own copy of the game (the Android APK, its asset dump and its master data). At the first
start the launcher checks those files and prepares them on the player's PC; the game's art and font the
launcher shows are made from them there too.

## What this port is made of

| Part | Where it comes from | License |
|---|---|---|
| Launcher (`launcher/`), setup and game patchers (`tools/`) | this project | AGPL-3.0 |
| Android runtime (`runtime/`) | [win.apk](https://github.com/israel2176/win.apk) (based on Mac.apk), commit `640f38f`, with this project's changes: [`winapk/winapk-nier.patch`](winapk/winapk-nier.patch) | AGPL-3.0, see `licenses/win.apk-*` |
| ANGLE, SwiftShader (in `runtime/bin`) | Chromium projects, built for win.apk | BSD-style, `licenses/LICENSES.chromium.html` |
| Java runtime (`runtime/jre`) | Eclipse Temurin 21 | GPLv2 with Classpath Exception, `runtime/jre/legal` |
| clang and lld (`runtime/clang`), the runtime's ahead-of-time compiler | LLVM, as packaged by win.apk | Apache-2.0 with LLVM exception, `licenses/clang` |
| Game server (`server/`, `server/bin/nier-server`) | [Lunar Tear](https://github.com/Walter-Sparrow/lunar-tear), the server (the revision Bunker pins), with the modifications of [Bunker](https://github.com/veritr1x/bunker) (`93afbea`, version 1.12.18: its overlays and patches), [Lunar Base](https://github.com/NMeliksah/lunar-base)'s grant engine and the [Lunar Tear Comfort Mod](https://github.com/NavHobbyDev/lunar-tear-comfort-mod)'s mods; this project's additions are listed in `server/PC-PORT.md` | MIT, `licenses/LunarTear-MIT.txt`, `licenses/Bunker-MIT.txt`, `licenses/LunarBase-MIT.txt` |
| Shop and event patch (`tools/masterdata_patcher`) | [lunar-tear-masterdata-patcher](https://github.com/NavHobbyDev/lunar-tear-masterdata-patcher) `9b6cd16` (from Walter Sparrow's lunar-scripts), as Bunker ships it | MIT, `licenses/lunar-tear-masterdata-patcher-MIT.txt` |
| APK patchers (`third_party/bunker`) | [Bunker](https://github.com/veritr1x/bunker) `android/tools`, unchanged | MIT, `licenses/Bunker-MIT.txt` |
| ASTC decoder (in the runtime) | texture2ddecoder, FP16 | MIT, `licenses/texture2ddecoder-MIT.txt`, `licenses/FP16-MIT.txt` |
| Python packages (`tools/venv`) | UnityPy, Pillow, texture2ddecoder, pycryptodome, fonttools, lz4 and msgpack, installed from PyPI at the first start (`tools/requirements-linux.txt`), not shipped | each package's own license, in its `*.dist-info` folder |

The runtime is a modified build of win.apk. As win.apk's trademark terms ask of forks, it is not
presented as win.apk or Mac.apk, and does not use their name as its own or their logo.

## Source code

Everything this port runs is built from the sources in this repository plus the upstream projects
named above. The runtime is win.apk at commit `640f38f06c4934664767abc365a43b3c6bd7dea3` with
`winapk/winapk-nier.patch` applied (see `winapk/README.md`).
