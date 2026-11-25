use serde_json::{json, Map, Value};

use crate::lang::tr;

pub const PADS: [&str; 16] = ["", "A", "X", "Y", "LB", "RB", "LT", "RT", "Back", "Start", "LS", "RS", "DPadUp", "DPadDown",
                              "DPadLeft", "DPadRight"];
pub const TOGGLES: [&str; 14] = ["Back", "Start", "LS", "RS", "LB", "RB", "LT", "RT", "X", "Y", "DPadUp", "DPadDown",
                                 "DPadLeft", "DPadRight"];

pub const QUALITY: [(&str, &str, &str, &str); 6] = [
    ("6", "Max", "bxSSDb6ZpCk%3D", "The screen's full size, every effect."),
    ("1", "Best", "PcUuO%2BMKZ9w%3D", "1080p, shadows, reflections, bloom and vignette."),
    ("2", "High", "0PXR2aAID%2Bw%3D", "720p with the effects."),
    ("3", "Medium", "oYD12QpSFkQ%3D", "540p, no shadows, reflections, bloom or vignette."),
    ("4", "Low", "EwkARg3Xkuw%3D", "540p, no effects: the lightest on the GPU."),
    ("5", "Power saving", "s78uC%2FowoEE%3D", "540p, no effects, at most 30 fps, to save power."),
];
pub fn quality(v: &str) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    QUALITY.iter().copied().find(|q| q.0 == v)
}
pub fn language(v: &str) -> Option<&'static str> {
    match v {
        "10" => Some("cWI5k7i3OLE%3D"),
        "22" => Some("242ZQj%2FEP%2FA%3D"),
        "23" => Some("wGmNosNTgUI%3D"),
        _ => None,
    }
}

pub fn default_binds() -> Value {
    let b = |name: &str, scene: &str, vk: u32, key: &str, pad: &str, x: f64, y: f64| {
        json!({"name": name, "scene": scene, "vk": vk, "key": key, "pad": pad, "x": x, "y": y})
    };
    Value::Array(vec![
        b("Interact / next", "field", 69, "E", "A", 0.775, 0.866),
        b("Tap centre", "field", 32, "Space", "", 0.5, 0.5),
        b("Mama", "field", 70, "F", "LS", 0.91, 0.84),
        b("Auto walk", "field", 82, "R", "", 0.941, 0.146),
        b("1st: skill I", "battle", 49, "1", "LB", 0.274, 0.787),
        b("1st: skill II", "battle", 50, "2", "LT", 0.323, 0.87),
        b("1st: charged", "battle", 51, "3", "X", 0.166, 0.829),
        b("2nd: skill I", "battle", 52, "4", "RB", 0.58, 0.787),
        b("2nd: skill II", "battle", 53, "5", "RT", 0.628, 0.87),
        b("2nd: charged", "battle", 54, "6", "Y", 0.471, 0.829),
        b("3rd: skill I", "battle", 55, "7", "DPadUp", 0.885, 0.787),
        b("3rd: skill II", "battle", 56, "8", "DPadDown", 0.932, 0.87),
        b("3rd: charged", "battle", 57, "9", "", 0.776, 0.829),
        b("Battle speed", "battle", 67, "C", "", 0.804, 0.034),
        b("Battle auto", "battle", 81, "Q", "", 0.893, 0.034),
        b("Battle menu", "battle", 9, "Tab", "Start", 0.967, 0.034),
        b("Yes", "dialog", 13, "Enter", "DPadRight", 0.805, 0.77),
        b("No", "dialog", 8, "Backspace", "DPadLeft", 0.445, 0.77),
        b("Skip story", "dialog", 75, "K", "RS", 0.128, 0.768),
    ])
}

pub fn default_move() -> Value {
    json!({"x": 0.2, "y": 0.7, "radius": 0.15, "keys": {"up": 87, "left": 65, "down": 83, "right": 68}})
}

pub fn defaults() -> Value {
    json!({
        "version": 6,
        "display": {"mode": "fullscreen", "size": "1600x900"},
        "graphics": {"backend": "vulkan", "adapter": "nvidia", "quality": "", "aa": "fxaa", "vkCompat": "auto",
                     "diagnostics": "off", "perf": "off"},
        "performance": {"engine": "aot", "fps": "60"},
        "custom": {"renderScale": "", "upscale": "", "aniso": "", "vsync": "", "idleFps": "",
                   "shadows": "", "reflections": "", "bloom": "", "vignette": "", "textures": ""},
        "language": {"text": "", "voice": "", "ui": ""},
        "mods": {"shopUnlimited": true, "staminaFree": false, "freeSkips": false, "unlimitedDailyPlays": false,
                 "ticketsToGems": false, "lostItemGems": false, "pityRewards": false, "noDuplicateFourStar": false,
                 "rewardMultiplier": "1", "expMultiplier": "1", "hideSaleNotices": true},
        "paths": {"runtime": "", "runtimeData": ""},
        "linux": {"runner": "auto", "proton": "", "env": ""},
        "controls": {"enabled": true, "move": default_move(), "binds": default_binds(),
                     "pad": {"mouseToggle": "Back", "mouseSpeed": 1}},
    })
}

pub fn merge(base: &Value, over: &Value) -> Value {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            let mut out = b.clone();
            for (k, v) in o {
                let merged = merge(b.get(k).unwrap_or(&Value::Null), v);
                out.insert(k.clone(), merged);
            }
            Value::Object(out)
        }
        (Value::Object(_), _) if over.is_null() => base.clone(),
        (_, o) if o.is_null() => base.clone(),
        (_, o) => o.clone(),
    }
}

pub fn get_str(s: &Value, path: &str) -> String {
    match s.pointer(&ptr(path)) {
        Some(Value::String(v)) => v.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Null) | None => "undefined".into(),
        Some(v) => v.to_string(),
    }
}
pub fn set(s: &mut Value, path: &str, v: Value) {
    let mut o = s;
    let keys: Vec<&str> = path.split('.').collect();
    for k in &keys[..keys.len() - 1] {
        if !o.get(*k).is_some_and(Value::is_object) {
            o[*k] = Value::Object(Map::new());
        }
        o = o.get_mut(*k).unwrap();
    }
    o[keys[keys.len() - 1]] = v;
}
pub fn ptr(path: &str) -> String {
    format!("/{}", path.replace('.', "/"))
}
pub fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn load(text: &str) -> (Value, bool, bool) {
    let fresh = text.trim().is_empty();
    let file: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut s = merge(&defaults(), &file);
    let mut changed = false;
    let version = s.get("version").and_then(Value::as_i64).unwrap_or(1);
    if version < 2 {
        s["controls"]["move"]["radius"] = json!(0.15);
    }
    if !["vulkan", "d3d11", "swiftshader"].contains(&get_str(&s, "graphics.backend").as_str()) {
        set(&mut s, "graphics.backend", json!("vulkan"));
        changed = true;
    }
    if !["", "10", "23"].contains(&get_str(&s, "language.text").as_str()) {
        set(&mut s, "language.text", json!("10"));
        changed = true;
    }
    if version < 4 {
        s["version"] = json!(4);
        s["controls"]["binds"] = default_binds();
        free_toggle(&mut s);
        changed = true;
    }
    if s["version"].as_i64().unwrap_or(4) < 5 {
        s["version"] = json!(5);
        let battle: Vec<Value> = default_binds().as_array().unwrap().iter()
            .filter(|b| b["scene"] == "battle").cloned().collect();
        let taken: Vec<String> = battle.iter().filter_map(|b| b["pad"].as_str()).filter(|p| !p.is_empty())
            .map(String::from).collect();
        let mut binds: Vec<Value> = s["controls"]["binds"].as_array().cloned().unwrap_or_default().into_iter()
            .filter(|b| b["scene"] != "battle")
            .map(|mut b| {
                if b["pad"].as_str().is_some_and(|p| taken.iter().any(|t| t == p)) {
                    b["pad"] = json!("");
                }
                b
            })
            .collect();
        binds.extend(battle);
        s["controls"]["binds"] = Value::Array(binds);
        free_toggle(&mut s);
        changed = true;
    }
    if s["version"].as_i64().unwrap_or(5) < 6 {
        s["version"] = json!(6);
        changed = true;
    }
    (s, fresh, changed)
}

pub fn free_toggle(s: &mut Value) {
    let t = get_str(s, "controls.pad.mouseToggle");
    if !TOGGLES.contains(&t.as_str()) {
        set(s, "controls.pad.mouseToggle", json!("Back"));
    }
    let t = get_str(s, "controls.pad.mouseToggle");
    if let Some(binds) = s["controls"]["binds"].as_array_mut() {
        for b in binds {
            if b["pad"].as_str() == Some(t.as_str()) {
                b["pad"] = json!("");
            }
        }
    }
}

pub fn mods_file(s: &Value) -> Value {
    let mut out = Map::new();
    for (k, v) in s["mods"].as_object().cloned().unwrap_or_default() {
        let value = if k.ends_with("Multiplier") {
            let n = match &v {
                Value::String(t) => t.parse::<f64>().ok(),
                Value::Number(n) => n.as_f64(),
                _ => None,
            }
            .filter(|n| *n != 0.0)
            .unwrap_or(1.0);
            json!(n)
        } else {
            json!(v == Value::Bool(true))
        };
        out.insert(k, value);
    }
    Value::Object(out)
}

pub fn launch_env(s: &Value) -> Vec<(String, String)> {
    let g = |p: &str| get_str(s, p);
    let mut env: Vec<(String, String)> = vec![
        ("JAVA_TOOL_OPTIONS".into(), "-Xss16m".into()),
        ("WINAPK_DIRECT_PRESENT".into(), "1".into()),
        ("WINAPK_ANGLE_BACKEND".into(), g("graphics.backend")),
        ("WINAPK_CURSOR".into(), "cursors\\nier.cur".into()),
    ];
    let mut put = |k: &str, v: String| {
        env.retain(|(n, _)| n != k);
        env.push((k.into(), v));
    };
    let set_or = |v: String| if v == "undefined" { String::new() } else { v };
    let runtime = set_or(g("paths.runtime"));
    if !runtime.is_empty() {
        put("NIER_RUNTIME", runtime);
    }
    let data = set_or(g("paths.runtimeData"));
    if !data.is_empty() {
        put("NIER_RUNTIME_DATA", data);
    }
    let adapter = set_or(g("graphics.adapter"));
    if !adapter.is_empty() {
        put("WINAPK_ANGLE_ADAPTER", adapter.clone());
        put("ANGLE_PREFERRED_DEVICE", adapter);
    }
    if g("display.mode") == "fullscreen" {
        put("WINAPK_FULLSCREEN", "1".into());
    } else {
        put("WINAPK_PHONE_SIZE", g("display.size"));
    }
    if g("performance.engine") == "jit" {
        put("WINAPK_ARM64_ENGINE", "jit".into());
    }
    let mut cap: Option<String> = None;
    if g("performance.fps") != "0" {
        cap = Some(g("performance.fps"));
    }
    if g("graphics.quality") == "5" && !cap.as_ref().and_then(|c| c.parse::<f64>().ok()).is_some_and(|c| c > 0.0 && c <= 30.0) {
        cap = Some("30".into());
    }
    if let Some(c) = cap {
        put("WINAPK_FPS_CAP", c);
    }
    if g("graphics.aa") == "fxaa" {
        put("WINAPK_FXAA", "1".into());
    }
    let vk = set_or(g("graphics.vkCompat"));
    put("WINAPK_VK_COMPAT", if vk.is_empty() { "auto".into() } else { vk });
    match g("graphics.perf").as_str() {
        "log" => put("WINAPK_PERF", "1".into()),
        "overlay" => put("WINAPK_PERF", "overlay".into()),
        _ => {}
    }
    let c = |k: &str| set_or(g(&format!("custom.{k}")));
    for (k, var) in [("upscale", "WINAPK_UPSCALE"), ("aniso", "WINAPK_ANISO"),
                     ("vsync", "WINAPK_VSYNC"), ("idleFps", "WINAPK_IDLE_FPS")] {
        if !c(k).is_empty() {
            put(var, c(k));
        }
    }
    let off: Vec<&str> = ["shadows", "reflections", "bloom", "vignette"].into_iter().filter(|k| c(k) == "off").collect();
    if !off.is_empty() {
        put("WINAPK_GAME_EFFECTS_OFF", off.join("+"));
    }
    let skip = match c("textures").as_str() {
        "half" => Some(1),
        "quarter" => Some(2),
        _ => None,
    };
    if let Some(skip) = skip {
        put("MACAPK_LAUNCH_ACTIVITY", "com.google.firebase.MessagingUnityPlayerActivity".into());
        put("MACAPK_LAUNCH_EXTRAS", format!("unity=-texture-mipmap-skip-level {skip}"));
    }
    if g("graphics.diagnostics") == "on" {
        put("WINAPK_GL_SHADER_LOG", "1".into());
        put("WINAPK_GL_DEPTH_TRACE", "1".into());
    }
    for line in g("linux.env").lines() {
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            if !k.is_empty() && !k.starts_with('#') {
                put(k, v.trim().into());
            }
        }
    }
    env
}

pub fn launch_prefs(s: &Value) -> Vec<(String, String)> {
    let mut p = Vec::new();
    if let Some(q) = quality(&get_str(s, "graphics.quality")) {
        p.push(("PerformanceLevel".into(), q.2.into()));
    }
    let text = get_str(s, "language.text");
    if text == "10" || text == "23" {
        p.push(("CurrentSystemLanguage".into(), language(&text).unwrap().into()));
    }
    if let Some(v) = language(&get_str(s, "language.voice")) {
        p.push(("CurrentVoiceLanguage".into(), v.into()));
    }
    p
}

pub fn summary(s: &Value) -> String {
    let d = if get_str(s, "display.mode") == "fullscreen" { tr("Full screen").into() } else { get_str(s, "display.size").replace('x', "×") };
    let q = tr(quality(&get_str(s, "graphics.quality")).map_or("In-game quality", |q| q.1));
    let fps = get_str(s, "performance.fps");
    let fps = if fps == "0" { tr("Unlimited").into() } else { format!("{fps} fps") };
    format!("{d} · {q} · {fps}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_settings_and_env() {
        let (s, fresh, _) = load("");
        assert!(fresh);
        let env = launch_env(&s);
        let has = |k: &str, v: &str| env.iter().any(|(a, b)| a == k && b == v);
        assert!(has("WINAPK_ANGLE_BACKEND", "vulkan"));
        assert!(has("WINAPK_FULLSCREEN", "1"));
        assert!(has("WINAPK_FPS_CAP", "60"));
        assert!(has("WINAPK_FXAA", "1"));
        let mut scaled = s.clone();
        set(&mut scaled, "custom.renderScale", json!("100"));
        assert!(!launch_env(&scaled).iter().any(|(k, _)| k == "WINAPK_RENDER_SCALE"));
        assert_eq!(mods_file(&s)["rewardMultiplier"], json!(1.0));
        assert_eq!(mods_file(&s)["shopUnlimited"], json!(true));
        let mut own = s.clone();
        set(&mut own, "linux.env", json!("MANGOHUD=1\n#OFF=1\nnot a variable\n DXVK_HUD = fps,memory \nWINAPK_FXAA=0"));
        let env = launch_env(&own);
        let has = |k: &str, v: &str| env.iter().filter(|(a, _)| a == k).map(|(_, b)| b.as_str()).eq([v]);
        assert!(has("MANGOHUD", "1") && has("DXVK_HUD", "fps,memory") && has("WINAPK_FXAA", "0"));
        assert_eq!(env.len(), launch_env(&s).len() + 2);
    }

    #[test]
    fn file_values_win_and_old_files_upgrade() {
        let (s, _, changed) = load(r#"{"version":3,"graphics":{"backend":"d3d11"},"controls":{"binds":[]}}"#);
        assert!(changed);
        assert_eq!(get_str(&s, "graphics.backend"), "d3d11");
        assert_eq!(s["version"], json!(6));
        assert_eq!(s["controls"]["binds"].as_array().unwrap().len(), 19);
        let mut s2 = s.clone();
        set(&mut s2, "graphics.quality", json!("5"));
        assert!(launch_env(&s2).contains(&("WINAPK_FPS_CAP".into(), "30".into())));
    }
}
