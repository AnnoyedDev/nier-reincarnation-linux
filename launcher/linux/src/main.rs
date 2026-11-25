mod lang;
mod settings;
mod style;
mod ui;
mod wine;

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use std::sync::mpsc::Sender;

use serde_json::Value;

use lang::{tr, trf};

static ROOT: OnceLock<PathBuf> = OnceLock::new();
static PAGE: OnceLock<Mutex<(Sender<String>, egui::Context)>> = OnceLock::new();
static BUSY: AtomicBool = AtomicBool::new(false);
static SAVES: AtomicBool = AtomicBool::new(false);
static EXPORTING: AtomicBool = AtomicBool::new(false);
static CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn root() -> &'static Path {
    ROOT.get().unwrap()
}
pub(crate) fn path(rel: &str) -> PathBuf {
    root().join(rel)
}

fn to_page(text: impl Into<String>) {
    if let Some(p) = PAGE.get() {
        let p = p.lock().unwrap();
        let _ = p.0.send(text.into());
        p.1.request_repaint();
    }
}
fn status(text: &str) {
    to_page(format!("status:{text}"));
}

fn read_text(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}
fn write_file(p: &Path, data: &[u8]) -> bool {
    let tmp = p.with_extension(format!("{}.tmp", p.extension().and_then(|e| e.to_str()).unwrap_or("")));
    fs::write(&tmp, data).is_ok() && fs::rename(&tmp, p).is_ok()
}

fn settings() -> Value {
    serde_json::from_str(&read_text(&path("settings.json"))).unwrap_or(Value::Null)
}
fn setting(s: &Value, ptr: &str) -> String {
    s.pointer(ptr).and_then(Value::as_str).unwrap_or("").to_string()
}

fn runner() -> Option<wine::Runner> {
    let s = settings();
    wine::choose(root(), &setting(&s, "/linux/runner"), &setting(&s, "/linux/proton"))
}

fn python() -> PathBuf {
    let venv = path("tools/venv/bin/python");
    if venv.is_file() { venv } else { PathBuf::from("python3") }
}

fn tied(c: &mut Command) -> &mut Command {
    unsafe {
        c.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        })
    }
}
fn track(child: &Child) {
    CHILDREN.lock().unwrap().push(child.id());
}
fn untrack(child: &Child) {
    CHILDREN.lock().unwrap().retain(|&p| p != child.id());
}
fn log_file(name: &str) -> Stdio {
    let _ = fs::create_dir_all(path("logs"));
    OpenOptions::new().create(true).append(true).open(path("logs").join(name)).map(Stdio::from).unwrap_or(Stdio::null())
}
fn terminate(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
}

fn kill_by_path(exe: &Path) {
    let Ok(rd) = fs::read_dir("/proc") else { return };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else { continue };
        if fs::read_link(e.path().join("exe")).ok().as_deref() == Some(exe) {
            unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..30 {
                if !e.path().exists() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if e.path().exists() {
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
    }
}

fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(200)).is_ok()
}

fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n / 2];
    if File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes)).is_err() {
        let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (t.wrapping_mul(2654435761).wrapping_add(i as u128 * 40503) >> (i % 64)) as u8;
        }
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Default)]
struct Launch {
    input_json: String,
    mods_json: String,
    env: Vec<(String, String)>,
    prefs: Vec<(String, String)>,
}

fn set_game_prefs(runtime_data: &Path, prefs: &[(String, String)]) {
    if prefs.is_empty() {
        return;
    }
    let file = runtime_data.join(
        "android/data/data/NieR-loopback/shared_prefs/com.square_enix.android_googleplay.nierspww.v2.playerprefs.xml");
    let mut xml = read_text(&file);
    if !xml.contains("</map>") {
        return;
    }
    for (name, value) in prefs {
        let open = format!("<string name=\"{name}\">");
        if let Some(at) = xml.find(&open) {
            let Some(end) = xml[at..].find("</string>").map(|e| at + e + 9) else { continue };
            let end = end + xml[end..].chars().take_while(|c| *c == '\r' || *c == '\n').count();
            let start = xml[..at].rfind('\n').map_or(at, |s| s + 1);
            xml.replace_range(start..end, "");
        }
        if !value.is_empty() {
            let map_end = xml.rfind("</map>").unwrap();
            xml.insert_str(map_end, &format!("    {open}{value}</string>\n"));
        }
    }
    write_file(&file, xml.as_bytes());
}

fn runtime_paths(env: &[(String, String)]) -> (PathBuf, PathBuf) {
    let mut runtime = path("runtime/bin/winapk-runtime.exe");
    let mut data = path("runtime-data");
    for (k, v) in env {
        if k == "NIER_RUNTIME" && !v.is_empty() {
            runtime = PathBuf::from(v);
        }
        if k == "NIER_RUNTIME_DATA" && !v.is_empty() {
            data = PathBuf::from(v);
        }
    }
    (runtime, data)
}

fn run_game(l: Launch) {
    start_game(l);
    BUSY.store(false, Ordering::SeqCst);
}

fn start_game(mut l: Launch) {
    let bin = path("server/bin");
    let (data, save) = (path("data"), path("save"));
    let _ = fs::create_dir_all(&save);

    let Some(runner) = runner() else {
        to_page("error:No Wine or Proton found. Install Steam's Proton, umu-launcher or Wine (Settings › System).");
        return;
    };

    status("Stopping old servers...");
    for name in ["nier-server", "auth-server", "octo-cdn", "lunar-tear"] {
        kill_by_path(&bin.join(name));
    }

    if read_text(&save.join("auth.secret")).trim().is_empty() {
        write_file(&save.join("auth.secret"), random_hex(64).as_bytes());
    }
    if !l.mods_json.is_empty() {
        write_file(&save.join("mods.json"), l.mods_json.as_bytes());
    }

    status("Preparing the game data...");
    if let Ok(mut p) = tied(Command::new(python()).arg("-u").arg(path("tools/patch_content.py"))
        .arg("--data").arg(&data).arg("--selection").arg(save.join("active-content.json"))
        .env("PYTHONUTF8", "1").env("PYTHONIOENCODING", "utf-8").current_dir(root())
        .stdout(log_file("content-patch.log")).stderr(log_file("content-patch.log")))
    .spawn()
    {
        track(&p);
        let _ = p.wait();
        untrack(&p);
    }

    status("Starting the server...");
    let server = tied(Command::new(bin.join("nier-server")).arg("--save").arg(&save).arg("--data").arg(&data)
        .current_dir(&data).stdout(log_file("server.log")).stderr(log_file("server.log")))
    .spawn();
    let mut server = match server {
        Ok(s) => s,
        Err(e) => {
            to_page(format!("error:{}", trf("The server did not start ({}).", &[&e])));
            return;
        }
    };
    track(&server);
    let mut ready = false;
    for _ in 0..600 {
        ready = port_open(3000) && port_open(8080) && port_open(8003);
        if ready || server.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let stop_server = |server: &mut Child| {
        terminate(server.id());
        let _ = server.wait();
        untrack(server);
    };
    if !ready {
        to_page("error:The server did not start. See logs/server.log.");
        stop_server(&mut server);
        return;
    }

    let (runtime, runtime_data) = runtime_paths(&l.env);
    status("Preparing Wine...");
    if let Err(e) = runner.prepare(&runtime, &runtime_data) {
        to_page(format!("error:{}", trf("The Wine prefix could not be prepared: {}", &[&e])));
        stop_server(&mut server);
        return;
    }

    if !l.input_json.is_empty() {
        write_file(&path("input-map.json"), l.input_json.as_bytes());
        l.env.push(("WINAPK_INPUT_MAP".into(), runner.win_path(&path("input-map.json"))));
    }
    let mut big_cursor = false;
    for (k, v) in l.env.iter_mut() {
        if k != "WINAPK_CURSOR" {
            continue;
        }
        *v = runner.win_path(&path("cursors/nier.cur"));
        big_cursor = true;
        for name in ["cursors/custom.ani", "cursors/custom.cur"] {
            if path(name).exists() {
                *v = runner.win_path(&path(name));
                big_cursor = false;
                break;
            }
        }
    }
    if big_cursor {
        l.env.push(("WINAPK_CURSOR_SCALE".into(), "1.5".into()));
    }
    set_game_prefs(&runtime_data, &l.prefs);

    status("Starting the game...");
    let args = vec!["--data".into(), runner.win_path(&runtime_data), "--apk".into(),
                    runner.win_path(&path("client/NieR-loopback.apk"))];
    let mut cmd = runner.command_in_port(&runner.win_path(&runtime), &args);
    cmd.stdout(log_file("game.log")).stderr(log_file("game.log"));
    for (k, v) in &l.env {
        if k == "WINEDLLOVERRIDES" {
            cmd.env(k, wine::with_ucrtbase(v));
        } else if !k.starts_with("NIER_") {
            cmd.env(k, v);
        }
    }
    match tied(&mut cmd).spawn() {
        Err(e) => to_page(format!("error:{}", trf("The game did not start ({}).", &[&e]))),
        Ok(mut game) => {
            track(&game);
            let code = game.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            untrack(&game);
            runner.kill_all();
            to_page(format!("exited:{code}"));
        }
    }
    stop_server(&mut server);
}

fn install_runtime() -> i32 {
    let Some(runner) = runner() else {
        eprintln!("No Wine or Proton found (install Steam's Proton, umu-launcher or Wine).");
        return 1;
    };
    let (runtime, data) = runtime_paths(&[]);
    let custom = setting(&settings(), "/paths/runtimeData");
    let data = if custom.is_empty() { data } else { PathBuf::from(custom) };
    if let Err(e) = runner.prepare(&runtime, &data) {
        eprintln!("The Wine prefix could not be prepared: {e}");
        return 1;
    }
    let args = vec!["--data".into(), runner.win_path(&data), "--apk".into(),
                    runner.win_path(&path("client/NieR-loopback.apk")), "--install-only".into()];
    let code = runner.command_in_port(&runner.win_path(&runtime), &args).status()
        .ok().and_then(|s| s.code()).unwrap_or(1);
    runner.kill_all();
    code
}

fn setup_ready() -> bool {
    if !path("client/NieR-loopback.apk").exists() || !path("data/assets/revisions/0/list.bin").exists() {
        return false;
    }
    if path("runtime-data/android").exists() {
        return true;
    }
    let custom = setting(&settings(), "/paths/runtimeData");
    !custom.is_empty() && Path::new(&custom).join("android").exists()
}

fn pick(kind: &str) -> Option<PathBuf> {
    let d = rfd::FileDialog::new();
    let d = match kind {
        "assets-folder" => return d.set_title(tr("The folder that holds revisions/0")).pick_folder(),
        "apk" => d.set_title(tr("NieR Re[in]carnation 3.7.1, arm64 APK")).add_filter(tr("Android game (.apk)"), &["apk"]),
        "assets" => d.set_title("resource_dump_android.7z").add_filter(tr("Asset dump (.7z, .zip)"), &["7z", "zip"]),
        "phone-save" => d.set_title(tr("A save backup from the phone (Bunker's Export save backup)"))
            .add_filter(tr("Save backup (.zip, game.db)"), &["zip", "db"]),
        "master" => d.set_title("20240404193219.bin.e").add_filter(tr("Master data (.bin.e)"), &["e"])
            .add_filter(tr("All files"), &["*"]),
        _ => d,
    };
    d.pick_file()
}

fn pick_save_path(title: &str, file: &str) -> Option<PathBuf> {
    let desktop = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Desktop")).filter(|p| p.is_dir());
    let mut d = rfd::FileDialog::new().set_title(tr(title)).set_file_name(file).add_filter(tr("Zip archive (.zip)"), &["zip"]);
    if let Some(dir) = desktop {
        d = d.set_directory(dir);
    }
    d.save_file()
}

fn stream_lines(cmd: &mut Command, prefix: &str) -> i32 {
    let mut child = match tied(cmd.stdout(Stdio::piped()).stderr(Stdio::piped())).spawn() {
        Ok(c) => c,
        Err(e) => {
            to_page(format!("setup-line:error:{prefix}{e}"));
            return -1;
        }
    };
    track(&child);
    let err = child.stderr.take().unwrap();
    let errs = std::thread::spawn(move || BufReader::new(err).lines().map_while(Result::ok).collect::<Vec<_>>());
    for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
        to_page(format!("setup-line:{}", line.trim_end_matches('\r')));
    }
    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
    untrack(&child);
    if code != 0 {
        if let Some(last) = errs.join().ok().and_then(|v| v.into_iter().rev().find(|l| !l.trim().is_empty())) {
            to_page(format!("setup-line:error:{prefix}{last}"));
        }
    }
    code
}

fn ensure_venv() -> bool {
    if path("tools/venv/bin/python").is_file() {
        return true;
    }
    to_page("setup-line:step:0/7:Installing the setup's Python packages (once)");
    let venv = path("tools/venv");
    let ok = stream_lines(Command::new("python3").args(["-m", "venv"]).arg(&venv), "Python 3 venv: ") == 0
        && stream_lines(Command::new(venv.join("bin/python")).args(["-m", "pip", "install", "--progress-bar", "off", "-r"])
            .arg(path("tools/requirements-linux.txt")), "pip: ") == 0;
    if !ok {
        let _ = fs::remove_dir_all(&venv);
    }
    ok
}

fn run_setup(apk: String, assets: String, master: String) {
    let code = if !ensure_venv() {
        -1
    } else {
        let exe = std::env::current_exe().unwrap_or_default();
        stream_lines(Command::new(python()).arg("-u").arg(path("tools/setup_game.py")).arg("--root").arg(root())
            .args(["--apk", &apk, "--assets", &assets, "--master", &master])
            .env("PYTHONUTF8", "1").env("PYTHONIOENCODING", "utf-8").env("NIER_LAUNCHER", exe)
            .current_dir(root()), "")
    };
    to_page(format!("setup-exit:{code}"));
    BUSY.store(false, Ordering::SeqCst);
}

fn date_stamp() -> String {
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

fn replace_name(text: &str, name: &str, with: &str) -> String {
    if name.len() < 3 {
        return text.to_string();
    }
    let (lower, needle) = (text.to_ascii_lowercase(), name.to_ascii_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(at) = lower[i..].find(&needle) {
        out.push_str(&text[i..i + at]);
        out.push_str(with);
        i += at + needle.len();
    }
    out.push_str(&text[i..]);
    out
}

fn hostname() -> String {
    read_text(Path::new("/proc/sys/kernel/hostname")).trim().to_string()
}

fn scrubbed(text: &str) -> String {
    let user = std::env::var("USER").unwrap_or_default();
    replace_name(&replace_name(text, &user, "USER"), &hostname(), "PC")
}

fn log_tail(file: &Path) -> String {
    const MAX: u64 = 16 << 20;
    let Ok(mut f) = File::open(file) else { return String::new() };
    let skipped = f.metadata().map_or(0, |m| m.len().saturating_sub(MAX));
    let mut bytes = Vec::new();
    if f.seek(SeekFrom::Start(skipped)).is_err() || f.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    if skipped == 0 {
        return text.into_owned();
    }
    let from = text.find('\n').map_or(0, |n| n + 1);
    format!("[export] the first {} bytes of this log were left out\n{}", skipped + from as u64, &text[from..])
}

fn meminfo(key: &str) -> u64 {
    read_text(Path::new("/proc/meminfo")).lines().find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0) / 1024
}

fn free_gb(p: &Path) -> u64 {
    let c = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).unwrap_or_default();
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return 0;
    }
    (s.f_bavail as u64 * s.f_frsize as u64) >> 30
}

struct Gpu {
    name: String,
    vendor: u32,
    vram: u64,
}

fn gpus() -> Vec<Gpu> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/sys/class/drm") else { return out };
    let mut cards: Vec<PathBuf> = rd.flatten().map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("card") && !n.contains('-')))
        .collect();
    cards.sort();
    for card in cards {
        let dev = card.join("device");
        let hex = |f: &str| u32::from_str_radix(read_text(&dev.join(f)).trim().trim_start_matches("0x"), 16).unwrap_or(0);
        let (vendor, device) = (hex("vendor"), hex("device"));
        if vendor == 0 {
            continue;
        }
        let mut vram = read_text(&dev.join("mem_info_vram_total")).trim().parse::<u64>().unwrap_or(0) >> 20;
        let slot = fs::read_link(&dev).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let mut name = slot.as_ref().and_then(|s| {
            let o = Command::new("lspci").args(["-mm", "-s", s]).output().ok()?;
            let line = String::from_utf8_lossy(&o.stdout).into_owned();
            let f: Vec<&str> = line.split('"').collect();
            let model = f.get(5)?;
            let model = model.find('[').and_then(|i| model[i + 1..].strip_suffix(']')).unwrap_or(model);
            let maker = match vendor { 0x1002 => "AMD", 0x10de => "NVIDIA", 0x8086 => "Intel", _ => f[3] };
            Some(format!("{maker} {model}"))
        }).unwrap_or_else(|| format!("GPU {vendor:04x}:{device:04x}"));
        if vendor == 0x10de {
            if let Ok(o) = Command::new("nvidia-smi").args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"]).output() {
                if let Some((n, m)) = String::from_utf8_lossy(&o.stdout).lines().next().and_then(|l| l.split_once(", ")) {
                    name = n.to_string();
                    vram = m.trim().parse().unwrap_or(vram);
                }
            }
        }
        name.retain(|c| c != '"' && c != '\\' && c >= ' ');
        out.push(Gpu { name, vendor, vram });
    }
    out
}

fn system_report() -> String {
    let os = read_text(Path::new("/etc/os-release")).lines().find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .unwrap_or("Linux").trim_matches('"').to_string();
    let kernel = read_text(Path::new("/proc/sys/kernel/osrelease")).trim().to_string();
    let cpu = read_text(Path::new("/proc/cpuinfo")).lines().find_map(|l| l.strip_prefix("model name"))
        .map(|l| l.trim_start_matches([' ', '\t', ':']).to_string()).unwrap_or_default();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let mut r = format!("NieR Re[in]carnation PC port (Linux, launcher {}): logs exported {}\n\n", env!("CARGO_PKG_VERSION"), date_stamp());
    r += &format!("System: {os}, kernel {kernel}, {desktop} on {session}\n");
    r += &format!("Processor: {cpu} ({threads} threads)\n");
    r += &format!("Memory: {} MB ({} MB free)\n", meminfo("MemTotal:"), meminfo("MemAvailable:"));
    r += &format!("Free space on the game's drive: {} GB\n", free_gb(root()));
    r += &format!("Wine: {}\n", runner().map(|w| w.describe()).unwrap_or_else(|| "none found".into()));
    r += "\nGPUs:\n";
    for (i, g) in gpus().iter().enumerate() {
        r += &format!("  {i}: {} (vendor 0x{:04X}), video memory {} MB\n", g.name, g.vendor, g.vram);
    }
    if let Ok(o) = Command::new("vulkaninfo").arg("--summary").output() {
        let text = String::from_utf8_lossy(&o.stdout);
        r += "\nVulkan:\n";
        for l in text.lines().filter(|l| l.contains("deviceName") || l.contains("driverInfo") || l.contains("apiVersion")) {
            r += &format!("  {}\n", l.trim());
        }
    }
    r
}

fn stage_text(src: &Path, dst: &Path) -> bool {
    src.is_file() && fs::write(dst, scrubbed(&log_tail(src))).is_ok()
}

fn make_logs_zip(out: &Path) -> Result<(), String> {
    let stage = std::env::temp_dir().join(format!("NieR-logs-{}", date_stamp()));
    let _ = fs::create_dir_all(stage.join("logs"));
    let _ = fs::create_dir_all(stage.join("runtime-logs"));
    let _ = fs::write(stage.join("system.txt"), scrubbed(&system_report()));
    let mut items = vec!["system.txt", "logs", "runtime-logs"];
    for f in ["settings.json", "setup.json"] {
        if stage_text(&path(f), &stage.join(f)) {
            items.push(f);
        }
    }
    for (from, to) in [(path("logs"), stage.join("logs")), (path("runtime-data/logs"), stage.join("runtime-logs"))] {
        for e in fs::read_dir(&from).into_iter().flatten().flatten() {
            if e.path().extension().is_some_and(|x| x == "log") {
                stage_text(&e.path(), &to.join(e.file_name()));
            }
        }
    }
    let ok = Command::new(python()).args(["-m", "zipfile", "-c", "logs.zip"]).args(&items).current_dir(&stage)
        .status().is_ok_and(|s| s.success());
    let moved = ok && (fs::rename(stage.join("logs.zip"), out).is_ok()
        || fs::copy(stage.join("logs.zip"), out).is_ok());
    let _ = fs::remove_dir_all(&stage);
    if moved { Ok(()) } else { Err("The zip could not be made.".into()) }
}

fn open(target: impl AsRef<std::ffi::OsStr>) {
    let _ = Command::new("xdg-open").arg(target).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}

fn export_logs(out: PathBuf) {
    match make_logs_zip(&out) {
        Ok(()) => {
            if let Some(dir) = out.parent() {
                open(dir);
            }
            to_page(format!("exported:{}", out.display()));
        }
        Err(e) => to_page(format!("export-error:{e}")),
    }
    EXPORTING.store(false, Ordering::SeqCst);
}

fn run_saves(request: String) {
    let (save, data) = (path("save"), path("data"));
    let _ = fs::create_dir_all(&save);
    let mut reply = String::new();
    if let Ok(mut child) = tied(Command::new(path("server/bin/nier-server")).arg("--admin").arg("--save").arg(&save)
        .arg("--data").arg(&data).current_dir(&data)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(log_file("saves.log")))
    .spawn()
    {
        track(&child);
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(request.as_bytes());
        }
        let _ = child.stdout.take().unwrap().read_to_string(&mut reply);
        let _ = child.wait();
        untrack(&child);
    }
    let reply = reply.trim_end();
    let reply = if reply.starts_with('{') {
        reply.to_string()
    } else {
        r#"{"ok":false,"error":"The save tool did not answer. See logs/saves.log."}"#.to_string()
    };
    SAVES.store(false, Ordering::SeqCst);
    to_page(format!("saves:{reply}"));
}

fn info() -> String {
    let files = path("data/assets/revisions/0/list.bin").exists();
    let save = fs::metadata(path("save/game.db")).map(|m| {
        let n = m.len();
        if n >= 1 << 20 { format!("{} MB", n >> 20) } else { format!("{} KB", n >> 10) }
    }).unwrap_or_default();
    let runner = runner();
    let found: Vec<String> = wine::protons().iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
    let v = serde_json::json!({
        "files": files, "save": save, "ready": setup_ready(), "ram": meminfo("MemTotal:"),
        "platform": "linux",
        "runner": runner.as_ref().map(|w| w.describe()).unwrap_or_default(),
        "prefix": runner.as_ref().map(|w| w.pfx().display().to_string()).unwrap_or_default(),
        "protons": found, "umu": wine::in_path("umu-run").is_some(), "wine": wine::in_path("wine").is_some(),
        "gpus": gpus().iter().map(|g| serde_json::json!({"name": g.name, "vendor": g.vendor, "vram": g.vram})).collect::<Vec<_>>(),
    });
    v.to_string()
}

pub(crate) fn message(msg: &str) {
    let (cmd, rest) = msg.split_once('\n').unwrap_or((msg, ""));
    match cmd {
        "load" => to_page(format!("settings:{}", read_text(&path("settings.json")))),
        "save" => {
            write_file(&path("settings.json"), rest.as_bytes());
        }
        "info" => {
            std::thread::spawn(|| to_page(format!("info:{}", info())));
        }
        "logs" => {
            let _ = fs::create_dir_all(path("logs"));
            open(path("logs"));
        }
        "prefix-folder" => {
            if let Some(w) = runner() {
                let _ = fs::create_dir_all(w.prefix());
                open(w.prefix());
            }
        }
        "export-logs" => {
            if EXPORTING.swap(true, Ordering::SeqCst) {
                return;
            }
            std::thread::spawn(|| {
                let Some(out) = pick_save_path("Save the logs for a bug report", &format!("NieR-logs-{}.zip", date_stamp()))
                else {
                    EXPORTING.store(false, Ordering::SeqCst);
                    return;
                };
                to_page("exporting:");
                export_logs(out);
            });
        }
        "quit" => end_children(),
        "pick" => {
            let kind = rest.to_string();
            std::thread::spawn(move || {
                if let Some(p) = pick(&kind) {
                    to_page(format!("picked:{kind}:{}", p.display()));
                }
            });
        }
        "setup" => {
            if BUSY.swap(true, Ordering::SeqCst) {
                return;
            }
            let mut l = rest.lines().map(str::to_string);
            let (a, b, c) = (l.next().unwrap_or_default(), l.next().unwrap_or_default(), l.next().unwrap_or_default());
            std::thread::spawn(move || run_setup(a, b, c));
        }
        "saves" => {
            let listing = ["\"op\":\"list\"", "\"op\":\"content-", "\"op\":\"images\""].iter().any(|k| rest.contains(k));
            if !listing && BUSY.load(Ordering::SeqCst) {
                return to_page(r#"saves:{"ok":false,"error":"The game is running. Close it before changing saves."}"#);
            }
            if SAVES.swap(true, Ordering::SeqCst) {
                return to_page(r#"saves:{"ok":false,"error":"Another save change is still running."}"#);
            }
            let req = rest.to_string();
            std::thread::spawn(move || run_saves(req));
        }
        "pick-save-export" => {
            let name: String = rest.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
            std::thread::spawn(move || {
                if let Some(out) = pick_save_path("Export the save for the phone (Bunker's Import save backup)",
                                                  &format!("NieR-save-{name}-{}.zip", date_stamp())) {
                    to_page(format!("picked:save-export:{}", out.display()));
                }
            });
        }
        "saves-folder" => {
            let dir = path("save/slot-backups");
            let _ = fs::create_dir_all(&dir);
            open(dir);
        }
        "play" => {
            if !setup_ready() {
                return to_page("error:Prepare the game first.");
            }
            if SAVES.load(Ordering::SeqCst) {
                return to_page("error:Wait for the save change to finish.");
            }
            if BUSY.swap(true, Ordering::SeqCst) {
                return;
            }
            let mut l = Launch::default();
            for line in rest.lines() {
                if let Some(v) = line.strip_prefix("input:") {
                    l.input_json = v.into();
                } else if let Some(v) = line.strip_prefix("mods:") {
                    l.mods_json = v.into();
                } else if let Some((k, v)) = line.strip_prefix("pref:").and_then(|s| s.split_once('=')) {
                    l.prefs.push((k.into(), v.into()));
                } else if let Some((k, v)) = line.strip_prefix("env:").and_then(|s| s.split_once('=')) {
                    l.env.push((k.into(), v.into()));
                }
            }
            std::thread::spawn(move || run_game(l));
        }
        _ => {}
    }
}

fn end_children() {
    for pid in CHILDREN.lock().unwrap().drain(..) {
        terminate(pid);
    }
    if let Some(w) = runner() {
        w.kill_all();
    }
}

fn main() {
    let exe = std::env::current_exe().expect("own path");
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    ROOT.set(std::env::var_os("NIER_ROOT").map(PathBuf::from).unwrap_or_else(|| exe.parent().unwrap().to_path_buf())).unwrap();

    unsafe {
        let mut l: libc::rlimit = std::mem::zeroed();
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut l) == 0 && l.rlim_cur < l.rlim_max {
            l.rlim_cur = l.rlim_max;
            libc::setrlimit(libc::RLIMIT_NOFILE, &l);
        }
    }

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--export-logs" {
        let out = std::path::absolute(&args[2]).unwrap_or_else(|_| PathBuf::from(&args[2]));
        std::process::exit(if make_logs_zip(&out).is_ok() { 0 } else { 1 });
    }
    if args.len() == 2 && args[1] == "--install-runtime" {
        std::process::exit(install_runtime());
    }

    let lock_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let lock = File::create(lock_dir.join("nier-launcher.lock")).expect("lock file");
    if unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        eprintln!("The launcher is already open.");
        return;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NieR Re[in]carnation")
            .with_app_id("nier-launcher")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    let result = eframe::run_native("NieR Re[in]carnation", options, Box::new(move |cc| {
        PAGE.set(Mutex::new((tx, cc.egui_ctx.clone()))).ok();
        Ok(Box::new(ui::App::new(cc, rx)))
    }));
    end_children();
    if let Err(e) = result {
        eprintln!("The window could not open: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_replaced_in_any_case() {
        assert_eq!(replace_name("/home/Alice/x alice", "alice", "USER"), "/home/USER/x USER");
        assert_eq!(replace_name("ab ab", "ab", "X"), "ab ab");
    }
}
