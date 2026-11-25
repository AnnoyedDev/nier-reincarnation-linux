#!/usr/bin/env bash
# Builds and assembles the Linux release folder:
#
#   nier-launcher            the launcher (launcher/linux, Rust + egui), native
#   cursors                  the game window's cursor (launcher/ui/game is made at setup)
#   server/bin/nier-server   Lunar Tear (with Bunker's changes), a native Linux build
#   tools/, third_party/     setup and the patchers (the system's Python 3, in tools/venv at first start)
#   runtime/                 the win.apk runtime (Windows: bin, lib, jre, clang), run in Wine/Proton
#   licenses/, README.md, LICENSE, NOTICE.md
#
# The runtime is the one Windows build: take it from the Windows release zip
# (its runtime/ folder) or from a win.apk build with winapk/winapk-nier.patch.
#
# Needs: go 1.22+, cargo.
# The player needs: Python 3, 7-Zip, and Proton (Steam), umu-launcher or Wine.
#
# usage: scripts/build_linux.sh --out <folder> --runtime <runtime folder, or an extracted Windows release>
set -euo pipefail
SRC=$(cd "$(dirname "$0")/.." && pwd)
OUT= RUNTIME=
while [ $# -gt 0 ]; do
  case $1 in
    --out) OUT=$2; shift 2 ;;
    --runtime) RUNTIME=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ -n "$OUT" ] && [ -n "$RUNTIME" ] || { sed -n '2,19p' "$0"; exit 2; }
[ -d "$RUNTIME/runtime/bin" ] && RUNTIME=$RUNTIME/runtime  # an extracted Windows release
[ -f "$RUNTIME/bin/winapk-runtime.exe" ] || { echo "no bin/winapk-runtime.exe in $RUNTIME" >&2; exit 1; }
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)

echo "== server"
(cd "$SRC/server" && CGO_ENABLED=0 go build -trimpath -ldflags "-s -w" -o "$OUT/server/bin/nier-server" ./cmd/nier-server)

echo "== launcher"
(cd "$SRC/launcher/linux" && cargo build --release --locked 2>/dev/null || cargo build --release)
install -m755 "$SRC/launcher/linux/target/release/nier-launcher" "$OUT/nier-launcher"
mkdir -p "$OUT/launcher/ui" "$OUT/cursors"
# launcher/ui/game: the title art, font and icons, made from the player's APK at setup.
cp "$SRC/launcher/cursors/nier.cur" "$OUT/cursors/"

echo "== runtime"
for part in bin lib jre clang; do
  mkdir -p "$OUT/runtime/$part"
  cp -a "$RUNTIME/$part/." "$OUT/runtime/$part/"
done
rm -f "$OUT/runtime/bin/winapk.png"

echo "== tools"
mkdir -p "$OUT/tools" "$OUT/third_party"
for f in setup_game.py make_client_apk.py fps_patch.py hide_slidepad.py patch_content.py requirements-linux.txt; do
  cp "$SRC/tools/$f" "$OUT/tools/"
done
cp -a "$SRC/tools/masterdata_patcher" "$SRC/tools/runtime-overlay" "$OUT/tools/"
cp -a "$SRC/third_party/bunker" "$OUT/third_party/"
find "$OUT/tools" "$OUT/third_party" -name __pycache__ -type d -prune -exec rm -rf {} +

echo "== documents"
cp -a "$SRC/licenses" "$OUT/"
cp "$SRC/README.md" "$SRC/LICENSE" "$SRC/NOTICE.md" "$OUT/"
mkdir -p "$OUT/data" "$OUT/save" "$OUT/logs"
echo "Assembled $OUT (start: ./nier-launcher)"
