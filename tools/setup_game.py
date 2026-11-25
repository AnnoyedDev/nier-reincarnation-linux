"""Prepares the port from the player's own copy of the game (NieR.exe runs it).

Nothing of the game ships with the port. The player gives three things:

  apk      the Android game, NieR Re[in]carnation 3.7.1 (Global), arm64 APK
  assets   the game's Android asset dump: resource_dump_android.7z, or a folder
           that already holds its revisions\\0 (used where it is, not copied)
  master   the master data, 20240404193219.bin.e

and this makes, under the port's folder:

  data\\assets\\revisions\\0     the assets (extracted, or a junction to the folder)
  data\\assets\\release         the master data
  client\\NieR-loopback.apk    the game patched for the local server and a PC
                               (make_client_apk.py, then hide_slidepad.py)
  runtime-data                 the game installed in the bundled runtime
  launcher\\ui\\game            the launcher's title art and font, from the APK
  setup.json                   what was done, for NieR.exe

On Linux (launcher/linux) the same, with the system's 7-Zip, a symlink for a
folder of assets, and the runtime step run in Wine by the launcher
(NIER_LAUNCHER --install-runtime), which also gives the game its /data.

Progress goes to stdout, one line each: "step:<i>/<n>:<what>",
"progress:<0-100>", "note:<text>", "error:<text>", and "done" at the end.

usage: python setup_game.py --root <port folder> --apk <file> --assets <7z or folder> --master <file>
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "third_party" / "bunker" / "android" / "tools"))

APK_SHA256 = "9ded0797ee607b6892b531ba3a02f4c4510774f5fe098ccada78ccff881c9209"  # 3.7.1-152, arm64
MASTER_NAME = "20240404193219.bin.e"
MASTER_SHA256 = "60555788242de9a0580b0d86f6d109d67bd9cabd16d7bcda9b9a99745f270778"
ASSET_BYTES = 21_000_000_000  # revisions\0
STEPS = 7
WINDOWS = os.name == "nt"


def say(kind, text=""):
    print(f"{kind}:{text}" if text else kind, flush=True)


def fail(text):
    say("error", text)
    sys.exit(1)


def sha256(path: Path, label: str) -> str:
    h = hashlib.sha256()
    size = path.stat().st_size
    done = 0
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 22), b""):
            h.update(block)
            done += len(block)
            say("progress", str(done * 100 // max(size, 1)))
    return h.hexdigest()


def junction(link: Path, target: Path):
    """A directory junction (no administrator rights needed); a symlink on Linux."""
    if link.exists() or link.is_symlink():
        return
    link.parent.mkdir(parents=True, exist_ok=True)
    if not WINDOWS:
        os.symlink(target, link, target_is_directory=True)
        return
    r = subprocess.run(["cmd", "/c", "mklink", "/J", str(link), str(target)], capture_output=True, text=True)
    if r.returncode != 0:
        fail(f"Could not link {link} to {target}: {r.stdout.strip()} {r.stderr.strip()}")


def assets_folder(path: Path):
    """The folder holding revisions\\0, for a folder the player gave."""
    for candidate in (path, path / "assets", path / "data" / "assets"):
        if (candidate / "revisions" / "0" / "list.bin").is_file():
            return candidate / "revisions" / "0"
    if (path / "list.bin").is_file() and path.name == "0":
        return path
    return None


def step_check(args):
    say("step", f"1/{STEPS}:Checking the game files")
    if not args.apk.is_file():
        fail(f"The APK was not found: {args.apk}")
    if sha256(args.apk, "apk") != APK_SHA256:
        fail("This APK is not NieR Re[in]carnation 3.7.1 (Global, arm64). Use the arm64 .apk of version 3.7.1-152.")
    if not args.master.is_file():
        fail(f"The master data was not found: {args.master}")
    if sha256(args.master, "master") != MASTER_SHA256:
        fail(f"This is not the game's master data {MASTER_NAME}.")
    if args.assets.is_dir():
        if not assets_folder(args.assets):
            fail("That folder does not hold the game's assets (revisions\\0\\list.bin).")
    elif not (args.assets.is_file() and args.assets.suffix.lower() in (".7z", ".zip")):
        fail("The assets are resource_dump_android.7z (or a .zip of it), or a folder holding its revisions\\0.")


def step_assets(args, root: Path):
    say("step", f"2/{STEPS}:Game assets")
    dest = root / "data" / "assets"
    if (dest / "revisions" / "0" / "list.bin").is_file():
        say("note", "The assets are already in place")
        return
    dest.mkdir(parents=True, exist_ok=True)
    if args.assets.is_dir():
        junction(dest / "revisions" / "0", assets_folder(args.assets))
        say("note", "Using the assets where they are")
        return
    free = shutil.disk_usage(dest).free
    if free < ASSET_BYTES + 2_000_000_000:
        fail(f"Extracting the assets needs about 23 GB free on that drive; {free / 1e9:.1f} GB is free. "
             "Free some space, or extract revisions\\0 elsewhere and choose that folder.")
    seven = seven_zip(root)
    # The dump's revisions\0 may sit inside a folder of the archive (a .7z or a .zip).
    prefix = archive_prefix(seven, args.assets)
    if prefix is None:
        fail("That archive does not hold the game's assets (revisions\\0\\list.bin).")
    sep = "\\" if WINDOWS else "/"
    cmd = [str(seven), "x", str(args.assets), f"-o{dest}", prefix.replace("\\", sep) + "revisions" + sep + "0",
           "-y", "-bsp1", "-bso0", "-bse1"]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    buf = b""
    last = -1
    while True:
        chunk = proc.stdout.read(256)
        if not chunk:
            break
        buf += chunk
        for m in re.finditer(rb"(\d+)%", buf):
            pct = int(m.group(1))
            if pct != last:
                last = pct
                say("progress", str(pct))
        buf = buf[-64:]
    code = proc.wait()
    if code == 0 and prefix:
        inner = dest / prefix.replace("\\", "/") / "revisions" / "0"
        if (inner / "list.bin").is_file():
            (dest / "revisions").mkdir(exist_ok=True)
            shutil.move(str(inner), str(dest / "revisions" / "0"))
            shutil.rmtree(dest / Path(prefix.replace("\\", "/")).parts[0], ignore_errors=True)
    if code != 0 or not (dest / "revisions" / "0" / "list.bin").is_file():
        fail("Extracting the assets failed: is the archive whole?")


def seven_zip(root: Path):
    """The bundled 7za.exe on Windows; the system's 7-Zip on Linux."""
    if WINDOWS:
        return root / "tools" / "7zip" / "7za.exe"
    for name in ("7z", "7zz", "7za"):
        found = shutil.which(name)
        if found:
            return Path(found)
    fail("7-Zip is needed to extract the assets: install 7zip (or p7zip), or extract revisions/0 and choose that folder.")


def archive_prefix(seven: Path, archive: Path):
    """The folder inside the archive that holds revisions\\0 ("" at the top), or None."""
    r = subprocess.run([str(seven), "l", "-slt", "-ba", str(archive)], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    tail = "revisions\\0\\list.bin"
    for line in r.stdout.splitlines():
        if line.startswith("Path = "):
            path = line[7:].replace("/", "\\")
            if path.lower().endswith(tail):
                return path[: len(path) - len(tail)]
    return None


def step_master(args, root: Path):
    say("step", f"3/{STEPS}:Master data")
    target = root / "data" / "assets" / "release" / MASTER_NAME
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.is_file():
        shutil.copy2(args.master, target)


def step_client(args, root: Path):
    say("step", f"4/{STEPS}:Preparing the game for the PC")
    out = root / "client" / "NieR-loopback.apk"
    if out.is_file():
        say("note", "The prepared game is already there")
        return
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(".apk.tmp")
    py = sys.executable
    say("progress", "10")
    r = subprocess.run([py, str(HERE / "make_client_apk.py"), str(args.apk), str(tmp)], capture_output=True, text=True)
    if r.returncode != 0:
        fail("Patching the game failed: " + (r.stderr.strip().splitlines() or ["?"])[-1])
    say("progress", "60")
    r = subprocess.run([py, str(HERE / "hide_slidepad.py"), str(tmp)], capture_output=True, text=True)
    if r.returncode != 0:
        # Only a picture: the game is ready without it. The APK goes back to how it
        # was before (hide_slidepad keeps a copy), and the full error goes to the logs.
        backup = tmp.with_name(tmp.name + ".slidepad.orig")
        if backup.is_file():
            os.replace(backup, tmp)
        logs = root / "logs"
        logs.mkdir(exist_ok=True)
        (logs / "setup-slidepad.log").write_text(
            f"python: {py}\nsys.path: {sys.path}\n\nstdout:\n{r.stdout}\n\nstderr:\n{r.stderr}\n", encoding="utf-8")
        last = (r.stderr.strip().splitlines() or ["?"])[-1]
        say("note", f"The touch stick stays visible (hiding it failed: {last}; details in logs\\setup-slidepad.log). "
                    "The game works the same.")
    for leftover in tmp.parent.glob(tmp.name + ".*"):
        leftover.unlink()
    for leftover in tmp.parent.glob("tmp*"):  # the patchers' work folders, released now
        if leftover.is_dir():
            shutil.rmtree(leftover, ignore_errors=True)
    os.replace(tmp, out)
    say("progress", "100")


def step_runtime(root: Path):
    say("step", f"5/{STEPS}:Installing the game in the runtime")
    data = root / "runtime-data"
    # Android's CPU and system files the game reads (the runtime has none).
    shutil.copytree(HERE / "runtime-overlay", data, dirs_exist_ok=True)
    if WINDOWS:
        runtime = root / "runtime" / "bin" / "winapk-runtime.exe"
        cmd = [str(runtime), "--data", str(data), "--apk", str(root / "client" / "NieR-loopback.apk"), "--install-only"]
    else:
        # The runtime is a Windows program: the launcher runs it in the port's Wine prefix.
        launcher = os.environ.get("NIER_LAUNCHER")
        if not launcher:
            fail("On Linux the setup runs from the launcher (nier-launcher).")
        say("note", "First start of Wine: making the prefix can take a minute")
        cmd = [launcher, "--install-runtime"]
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=str(root))
    if r.returncode not in (0, 3):
        tail = (r.stderr or r.stdout).strip().splitlines()[-3:]
        fail("Installing the game failed: " + " / ".join(tail))
    if not WINDOWS:
        return  # the launcher's Wine drive gives the game its /data (launcher/linux/src/wine.rs)
    # Java reads the game's files by their Android paths (/data/...), which
    # Windows resolves on the current drive: <drive>:\data leads there.
    link = Path(root.anchor) / "data"
    target = data / "android" / "data"
    if not link.exists():
        junction(link, target)
    elif os.path.realpath(link) != os.path.realpath(target):
        say("note", f"{link} already exists and is not the port's: the game's videos may not play")


def step_launcher_art(args, root: Path):
    say("step", f"6/{STEPS}:The launcher's title and font, from the game")
    out = root / "launcher" / "ui" / "game"
    out.mkdir(parents=True, exist_ok=True)
    import UnityPy
    import game_fonts
    import texture2ddecoder
    from PIL import Image

    blocks = {48: 4, 49: 5, 50: 6, 51: 8, 54: 4, 55: 5, 56: 6, 57: 8}

    def decode(t):
        b = blocks[int(t.m_TextureFormat)]
        w, h = t.m_Width, t.m_Height
        n = ((w + b - 1) // b) * ((h + b - 1) // b) * 16
        raw = texture2ddecoder.decode_astc(bytes(t.image_data)[:n], w, h, b, b)
        return Image.frombytes("RGBA", (w, h), raw, "raw", "BGRA").transpose(Image.FLIP_TOP_BOTTOM)

    with zipfile.ZipFile(args.apk) as apk, tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as work:
        work = Path(work)
        files = []
        for name in apk.namelist():
            if name.startswith("assets/bin/Data/") and "/Managed/" not in name and "/" not in name[16:]:
                path = work / name[16:]
                path.write_bytes(apk.read(name))
                files.append(path)
        say("progress", "30")
        for path in files:  # the game's menu font
            try:
                fonts = game_fonts.read_fonts(path)
            except Exception:
                continue
            if "FOT-RodinPro-DB" in fonts:
                (out / "Rodin.otf").write_bytes(fonts["FOT-RodinPro-DB"])
                break
        say("progress", "50")
        env = UnityPy.load(str(work))
        art = {}
        for obj in env.objects:
            if obj.type.name == "Texture2D" and obj.peek_name() in ("title_keyvisual", "dark_logo_black"):
                t = obj.read()
                if t.image_data and t.m_Name not in art:
                    art[t.m_Name] = decode(t)
        if len(art) == 2:
            kv, logo = art["title_keyvisual"], art["dark_logo_black"]
            # 16:9 around the figures, the logo where the game's title has it.
            top = (kv.height - kv.width * 9 // 16) // 2
            scene = kv.crop((0, top, kv.width, top + kv.width * 9 // 16)).resize((1920, 1080), Image.LANCZOS)
            lw = int(1920 * 0.56)
            mark = logo.resize((lw, int(logo.height * lw / logo.width)), Image.LANCZOS)
            scene.alpha_composite(mark, ((1920 - lw) // 2, int(1080 * 0.40) - mark.height // 2))
            scene.convert("RGB").save(out / "title.jpg", quality=90)
        else:
            say("note", "The title art was not found; the launcher's title screen has no picture")
    say("progress", "100")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--apk", type=Path, required=True)
    p.add_argument("--assets", type=Path, required=True)
    p.add_argument("--master", type=Path, required=True)
    args = p.parse_args()
    root = args.root.resolve()
    step_check(args)
    step_assets(args, root)
    step_master(args, root)
    step_client(args, root)
    step_runtime(root)
    step_launcher_art(args, root)
    say("step", f"7/{STEPS}:Done")
    (root / "setup.json").write_text(json.dumps({
        "version": 1, "apk_sha256": APK_SHA256, "master": MASTER_NAME,
        "assets": "extracted" if args.assets.is_file() else str(args.assets.resolve()),
    }, indent=2), encoding="utf-8")
    say("done")


if __name__ == "__main__":
    main()
