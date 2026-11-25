use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DRIVE: &str = "n:";

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Proton,
    Umu,
    Wine,
}

#[derive(Clone, Debug)]
pub struct Runner {
    pub kind: Kind,
    pub proton: Option<PathBuf>,
    pub root: PathBuf,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

pub fn in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| Path::new(d).join(name)).find(|p| p.is_file())
}

fn steam_roots() -> Vec<PathBuf> {
    let h = home();
    let mut v = vec![h.join(".steam/steam"), h.join(".steam/root"), h.join(".local/share/Steam"),
                     h.join(".var/app/com.valvesoftware.Steam/data/Steam")];
    v.retain(|p| p.is_dir());
    v.dedup_by(|a, b| fs::canonicalize(&*a).ok() == fs::canonicalize(&*b).ok());
    v
}

fn version_key(name: &str) -> Vec<(u8, u64, String)> {
    let mut out = Vec::new();
    let mut num = String::new();
    let mut text = String::new();
    for c in name.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            if !text.is_empty() {
                out.push((0, 0, std::mem::take(&mut text)));
            }
            num.push(c);
        } else {
            if !num.is_empty() {
                out.push((1, num.parse().unwrap_or(0), String::new()));
                num.clear();
            }
            if c != '\0' {
                text.push(c.to_ascii_lowercase());
            }
        }
    }
    out
}

pub fn protons() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from("/usr/share/steam/compatibilitytools.d")];
    for s in steam_roots() {
        dirs.push(s.join("compatibilitytools.d"));
        dirs.push(s.join("steamapps/common"));
    }
    for d in dirs {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            if e.path().join("proton").is_file() && !found.iter().any(|p: &PathBuf| p.file_name() == e.path().file_name()) {
                found.push(e.path());
            }
        }
    }
    let rank = |p: &PathBuf| {
        let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let tier = if n.starts_with("GE-Proton") { 0 } else if n.contains("Experimental") { 1 } else { 2 };
        (tier, std::cmp::Reverse(version_key(&n)))
    };
    found.sort_by_key(rank);
    found
}

pub fn choose(root: &Path, runner: &str, proton: &str) -> Option<Runner> {
    let custom = (!proton.is_empty()).then(|| PathBuf::from(proton)).filter(|p| p.join("proton").is_file());
    let proton = custom.or_else(|| protons().into_iter().next());
    let umu = in_path("umu-run").is_some();
    let wine = in_path("wine").is_some();
    let kind = match runner {
        "proton" if proton.is_some() => Kind::Proton,
        "umu" if umu => Kind::Umu,
        "wine" if wine => Kind::Wine,
        _ if umu && proton.is_some() => Kind::Umu,
        _ if proton.is_some() => Kind::Proton,
        _ if wine => Kind::Wine,
        _ if umu => Kind::Umu,
        _ => return None,
    };
    Some(Runner { kind, proton, root: root.to_path_buf() })
}

impl Runner {
    pub fn prefix(&self) -> PathBuf {
        self.root.join("prefix")
    }
    pub fn pfx(&self) -> PathBuf {
        self.prefix().join("pfx")
    }

    pub fn describe(&self) -> String {
        let name = self.proton.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        match self.kind {
            Kind::Proton => format!("Proton ({})", name.unwrap_or_default()),
            Kind::Umu => format!("umu-run ({})", name.unwrap_or_else(|| "UMU-Proton".into())),
            Kind::Wine => {
                let v = Command::new("wine").arg("--version").output().ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
                format!("Wine ({v})")
            }
        }
    }

    pub fn command(&self, exe: &str, args: &[String]) -> Command {
        let mut c;
        match self.kind {
            Kind::Proton => {
                c = Command::new(self.proton.as_ref().unwrap().join("proton"));
                c.arg("run");
                c.env("STEAM_COMPAT_DATA_PATH", self.prefix());
                c.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam_roots().into_iter().next().unwrap_or_else(|| self.prefix()));
            }
            Kind::Umu => {
                c = Command::new("umu-run");
                c.env("WINEPREFIX", self.pfx());
                c.env("GAMEID", "umu-default");
                if let Some(p) = &self.proton {
                    c.env("PROTONPATH", p);
                }
            }
            Kind::Wine => {
                c = Command::new("wine");
                c.env("WINEPREFIX", self.pfx());
            }
        }
        c.env("WINEDEBUG", std::env::var("WINEDEBUG").unwrap_or_else(|_| "-all".into()));
        c.env("WINEDLLOVERRIDES", with_ucrtbase(&std::env::var("WINEDLLOVERRIDES").unwrap_or_default()));
        c.arg(exe).args(args);
        c
    }

    pub fn command_in_port(&self, exe: &str, args: &[String]) -> Command {
        let mut all: Vec<String> = ["/d", "N:\\port", "/b", "/wait", exe].map(String::from).to_vec();
        all.extend_from_slice(args);
        let mut c = self.command("start.exe", &all);
        c.current_dir(&self.root);
        c
    }

    fn wineserver(&self) -> PathBuf {
        if let Some(p) = &self.proton {
            for sub in ["files/bin/wineserver", "dist/bin/wineserver"] {
                if p.join(sub).is_file() {
                    return p.join(sub);
                }
            }
        }
        PathBuf::from("wineserver")
    }

    pub fn kill_all(&self) {
        if self.pfx().is_dir() {
            let _ = Command::new(self.wineserver()).arg("-k").env("WINEPREFIX", self.pfx()).status();
        }
    }

    pub fn prepare(&self, runtime: &Path, runtime_data: &Path) -> Result<(), String> {
        if let Some(bin) = runtime.parent() {
            let (ours, jre) = (bin.join("ucrtbase.dll"), bin.join("../jre/bin/ucrtbase.dll"));
            if !ours.exists() && jre.is_file() {
                fs::copy(&jre, &ours).map_err(|e| format!("{}: {e}", ours.display()))?;
            }
        }
        if !self.pfx().join("dosdevices").is_dir() {
            fs::create_dir_all(self.prefix()).map_err(|e| e.to_string())?;
            let status = self.command("wineboot.exe", &["-u".into()]).status()
                .map_err(|e| format!("could not run {:?}: {e}", self.kind))?;
            if !self.pfx().join("dosdevices").is_dir() {
                return Err(format!("the Wine prefix was not made ({status})"));
            }
        }
        let drive = self.root.join(".wine-drive");
        fs::create_dir_all(&drive).map_err(|e| e.to_string())?;
        relink(&drive.join("port"), &self.root)?;
        relink(&drive.join("data"), &runtime_data.join("android").join("data"))?;
        relink(&self.pfx().join("dosdevices").join(DRIVE), &drive)?;
        Ok(())
    }

    pub fn win_path(&self, p: &Path) -> String {
        let s = match p.strip_prefix(&self.root) {
            Ok(rel) => format!("N:\\port\\{}", rel.display()),
            Err(_) => format!("Z:{}", p.display()),
        };
        s.replace('/', "\\").trim_end_matches('\\').to_string()
    }
}

pub fn with_ucrtbase(overrides: &str) -> String {
    match overrides {
        "" => "ucrtbase=n,b".into(),
        o if o.contains("ucrtbase") => o.into(),
        o => format!("{o};ucrtbase=n,b"),
    }
}

fn relink(link: &Path, target: &Path) -> Result<(), String> {
    if let Ok(m) = fs::symlink_metadata(link) {
        if !m.file_type().is_symlink() {
            return Err(format!("{} exists and is not a link", link.display()));
        }
        if fs::read_link(link).ok().as_deref() == Some(target) {
            return Ok(());
        }
        fs::remove_file(link).map_err(|e| e.to_string())?;
    }
    symlink(target, link).map_err(|e| format!("{}: {e}", link.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_sort_as_numbers() {
        assert!(version_key("GE-Proton11-11") > version_key("GE-Proton11-7"));
        assert!(version_key("GE-Proton10-34") < version_key("GE-Proton11-1"));
    }

    #[test]
    fn ucrtbase_stays_native() {
        assert_eq!(with_ucrtbase(""), "ucrtbase=n,b");
        assert_eq!(with_ucrtbase("dxgi=n"), "dxgi=n;ucrtbase=n,b");
        assert_eq!(with_ucrtbase("ucrtbase=b"), "ucrtbase=b");
    }

    #[test]
    fn windows_paths() {
        let r = Runner { kind: Kind::Wine, proton: None, root: PathBuf::from("/games/nier") };
        assert_eq!(r.win_path(Path::new("/games/nier/client/a.apk")), "N:\\port\\client\\a.apk");
        assert_eq!(r.win_path(Path::new("/games/nier")), "N:\\port");
        assert_eq!(r.win_path(Path::new("/mnt/x/y")), "Z:\\mnt\\x\\y");
    }
}
