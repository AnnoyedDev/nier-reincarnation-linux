use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui};
use serde_json::{json, Value};

use crate::lang::{self, tr, trf};
use crate::settings::{self as st, get_str};
use crate::style::{self as s, *};

#[derive(Clone, Copy, PartialEq)]
enum View {
    Home,
    Settings,
    Events,
    Controls,
}
const SUBS: [(&str, &str); 7] = [("display", "Display"), ("graphics", "Graphics"), ("performance", "Performance"),
                                 ("language", "Language"), ("mods", "Mods"), ("saves", "Saves"), ("system", "System")];

#[derive(Clone, Copy, PartialEq)]
enum Sel {
    Move,
    Bind(usize),
}

enum Tag {
    List,
    Do { done: Done, after: After },
    Images(Vec<String>, PicsFor),
    ContentList,
    ContentSet,
}
enum Done {
    Text(String),
    Delete(String),
    Backup(String),
    Apply(String),
}
enum After {
    None,
    CloseEditor,
    Editor(String),
    Applied,
}
#[derive(Clone, Copy)]
enum PicsFor {
    Editor,
    Events,
}

enum Ask {
    Use(String),
    Rename(String),
    Delete(String),
    Restart,
    NewEmpty,
    PhoneImport(String),
    SetAllShown,
    Drop(Then),
    Apply,
    PlayAnyway,
}
enum Then {
    Use(String),
    Edit(String),
    Restart,
}
struct Modal {
    title: String,
    text: String,
    input: Option<String>,
    ok: String,
    ask: Ask,
    focus: bool,
}

struct Editor {
    id: String,
    player: Value,
    catalog: Value,
    char_name: HashMap<i64, String>,
}
#[derive(Default)]
struct Pending {
    kinds: BTreeMap<&'static str, BTreeMap<i64, i64>>,
    amounts: BTreeMap<String, i64>,
}
impl Pending {
    fn count(&self) -> usize {
        self.kinds.values().map(BTreeMap::len).sum::<usize>() + self.amounts.len()
    }
    fn map(&mut self, kind: &'static str) -> &mut BTreeMap<i64, i64> {
        self.kinds.entry(kind).or_default()
    }
    fn get(&self, kind: &str) -> Option<&BTreeMap<i64, i64>> {
        self.kinds.get(kind)
    }
}
const ED_KINDS: [(&str, &str); 5] = [("costumes", "Characters"), ("weapons", "Weapons"), ("companions", "Companions"),
                                     ("materials", "Materials"), ("consumables", "Items")];
const LEVEL_KINDS: [&str; 3] = ["costumes", "weapons", "companions"];
const ROW_LIMIT: usize = 400;

struct Item {
    id: i64,
    name: String,
    sub: String,
    img: String,
    rarity: i64,
    have: Option<Value>,
}

#[derive(Default)]
struct CtFilter {
    search: String,
    group: String,
    state: String,
}

struct Pics {
    tex: HashMap<String, Option<TextureHandle>>,
    ask: Sender<String>,
    got: Receiver<(String, Option<egui::ColorImage>)>,
}
impl Pics {
    fn new(ctx: egui::Context) -> Self {
        let (ask, rx) = channel::<String>();
        let (tx, got) = channel();
        std::thread::spawn(move || {
            for path in rx {
                let img = image::open(crate::path(&path)).ok().map(|i| {
                    let i = i.to_rgba8();
                    egui::ColorImage::from_rgba_unmultiplied([i.width() as usize, i.height() as usize], &i)
                });
                let _ = tx.send((path, img));
                ctx.request_repaint();
            }
        });
        Pics { tex: HashMap::new(), ask, got }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((path, img)) = self.got.try_recv() {
            let t = img.map(|i| ctx.load_texture(&path, i, egui::TextureOptions::LINEAR));
            self.tex.insert(path, t);
        }
    }
    fn get(&mut self, path: &str) -> Option<TextureHandle> {
        match self.tex.get(path) {
            Some(t) => t.clone(),
            None => {
                self.tex.insert(path.to_string(), None);
                let _ = self.ask.send(path.to_string());
                None
            }
        }
    }
    fn forget(&mut self, path: &str) {
        self.tex.remove(path);
    }
}

pub struct App {
    rx: Receiver<String>,
    st: Value,
    loaded: bool,
    fresh: bool,
    save_at: Option<Instant>,
    view: View,
    sub: &'static str,
    ready: Option<bool>,
    gpu_checked: bool,
    status: String,
    status_error: bool,
    files: (String, Option<bool>),
    server_on: bool,
    save_label: String,
    play_disabled: bool,
    game_running: bool,
    runner_note: String,
    choice: [String; 3],
    setup_running: bool,
    setup_step: (f32, f32),
    setup_bar: f32,
    setup_text: String,
    setup_error: String,
    export_note: (String, bool),
    selected: Sel,
    scene: &'static str,
    listening: Option<usize>,
    slots: Vec<Value>,
    active: String,
    saves_running: bool,
    queue: VecDeque<(Value, Tag)>,
    saves_busy: bool,
    saves_msg: (String, bool),
    ed: Option<Editor>,
    ed_kind: &'static str,
    pending: Pending,
    ed_search: String,
    ed_rarity: String,
    ed_owned: String,
    ed_bulk_level: String,
    edits: HashMap<(String, i64), String>,
    amount_edits: HashMap<String, String>,
    modal: Option<Modal>,
    export_id: Option<String>,
    pic_have: HashSet<String>,
    pic_asked: HashSet<String>,
    ct_loaded: bool,
    ct_banners: Vec<Value>,
    ct_events: Vec<Value>,
    ct_save_at: Option<Instant>,
    ct_msg: (String, bool),
    ct_filter: [CtFilter; 2],
    pics: Pics,
    started: Instant,
}

fn send(cmd: &str, rest: &str) {
    crate::message(&if rest.is_empty() { cmd.to_string() } else { format!("{cmd}\n{rest}") });
}

impl App {
    pub fn new(cc: &eframe::CreationContext, rx: Receiver<String>) -> Self {
        fonts(&cc.egui_ctx);
        cc.egui_ctx.all_styles_mut(|st| {
            st.visuals = s::visuals();
            st.spacing.item_spacing = vec2(8.0, 8.0);
            st.spacing.scroll = egui::style::ScrollStyle::thin();
            st.interaction.selectable_labels = false;
        });
        send("load", "");
        send("info", "");
        let mut app = App {
            rx, st: st::defaults(), loaded: false, fresh: false, save_at: None, view: View::Home,
            sub: "display", ready: None, gpu_checked: false, status: "READY".into(), status_error: false,
            files: ("Checking".into(), None), server_on: false, save_label: "—".into(), play_disabled: false,
            game_running: false,
            runner_note: "Only the game's runtime runs in Wine; the launcher, the server and the setup are native.".into(),
            choice: Default::default(), setup_running: false, setup_step: (0.0, 1.0), setup_bar: 0.0,
            setup_text: "Choose the three files.".into(), setup_error: String::new(),
            export_note: ("Export logs saves them in one zip to send with a bug report, with your system and GPU details. Your user and PC names are left out.".into(), false),
            selected: Sel::Move, scene: "field", listening: None,
            slots: Vec::new(), active: String::new(), saves_running: false, queue: VecDeque::new(), saves_busy: false,
            saves_msg: Default::default(), ed: None, ed_kind: "costumes", pending: Pending::default(),
            ed_search: String::new(), ed_rarity: String::new(), ed_owned: String::new(), ed_bulk_level: String::new(),
            edits: HashMap::new(), amount_edits: HashMap::new(), modal: None, export_id: None,
            pic_have: HashSet::new(), pic_asked: HashSet::new(),
            ct_loaded: false, ct_banners: Vec::new(), ct_events: Vec::new(), ct_save_at: None, ct_msg: Default::default(),
            ct_filter: Default::default(), pics: Pics::new(cc.egui_ctx.clone()), started: Instant::now(),
        };
        app.saves_call(json!({"op": "list"}), Tag::List);
        if let Ok(page) = std::env::var("NIER_PAGE") {
            let (view, sub) = page.split_once('/').unwrap_or((&page, ""));
            match view {
                "settings" => app.go(View::Settings),
                "events" => app.go(View::Events),
                "controls" => app.go(View::Controls),
                _ => {}
            }
            if let Some(k) = SUBS.iter().find(|x| x.0 == sub) {
                app.sub = k.0;
            }
            app.scene = match sub { "battle" => "battle", "dialog" => "dialog", _ => "field" };
        }
        app
    }

    fn get(&self, path: &str) -> String {
        get_str(&self.st, path)
    }
    fn set(&mut self, path: &str, v: Value) {
        st::set(&mut self.st, path, v);
        self.save();
    }
    fn save(&mut self) {
        self.save_at = Some(Instant::now() + Duration::from_millis(250));
    }
    fn flush_save(&mut self) {
        if self.save_at.take().is_some() && self.loaded {
            send("save", &serde_json::to_string_pretty(&self.st).unwrap_or_default());
        }
    }
    fn set_status(&mut self, text: &str, error: bool) {
        self.status = text.to_string();
        self.status_error = error;
    }

    fn choice(&mut self, ui: &mut Ui, key: &str, options: &[(&str, &str)]) {
        let current = self.get(key);
        if let Some(v) = choice_grid(ui, options, &current) {
            let value = match v.as_str() {
                "true" => json!(true),
                "false" => json!(false),
                _ => json!(v),
            };
            self.set(key, value);
        }
    }
    fn path_field(&mut self, ui: &mut Ui, label: &str, key: &str, hint: &str) {
        let mut v = self.get(key);
        if v == "undefined" {
            v.clear();
        }
        if field(ui, label, &mut v, hint).changed() {
            self.set(key, json!(v.trim()));
        }
    }

    fn on_message(&mut self, ctx: &egui::Context, msg: String) {
        let (kind, text) = msg.split_once(':').unwrap_or((&msg, ""));
        let text = text.to_string();
        match kind {
            "settings" => {
                let (v, fresh, changed) = st::load(&text);
                self.st = v;
                self.fresh = fresh;
                self.loaded = true;
                if changed {
                    self.save();
                }
                lang::set(&self.get("language.ui"));
            }
            "info" => {
                let Ok(info) = serde_json::from_str::<Value>(&text) else { return };
                self.auto_for_gpu(&info);
                self.ready = Some(info["ready"].as_bool().unwrap_or(false));
                self.runner_note(&info);
                if self.ready == Some(true) {
                    let files = info["files"].as_bool().unwrap_or(false);
                    self.files = (if files { "Ready" } else { "Missing" }.into(), Some(files));
                    let save = info["save"].as_str().unwrap_or("");
                    if self.slots.is_empty() {
                        self.save_label = if save.is_empty() { "New".into() } else { format!("game.db · {save}") };
                    }
                }
            }
            "picked" => {
                let (what, where_) = text.split_once(':').unwrap_or((&text, ""));
                let where_ = where_.to_string();
                match what {
                    "phone-save" => self.ask("Import from phone", "The backup becomes a new save; your saves here stay as they are. Name of the new save:",
                                             Some(tr("From phone")), "Import", Ask::PhoneImport(where_)),
                    "save-export" => {
                        if let Some(id) = self.export_id.take() {
                            let name = self.slot_name(&id);
                            let file = where_.rsplit(['/', '\\']).next().unwrap_or("").to_string();
                            self.saves_do(json!({"op": "export", "id": id, "path": where_}),
                                          Done::Text(trf("\"{}\" was exported to {}. On the phone, use Bunker's Import save backup.", &[&name, &file])), After::None);
                        }
                    }
                    _ => {
                        let i = match what {
                            "apk" => 0,
                            "assets" | "assets-folder" => 1,
                            _ => 2,
                        };
                        self.choice[i] = where_;
                    }
                }
            }
            "setup-line" => self.setup_line(&text),
            "setup-exit" => {
                self.setup_running = false;
                if text == "0" {
                    self.setup_bar = 1.0;
                    send("info", "");
                    self.pics.forget("launcher/ui/game/title.jpg");
                    fonts(ctx);
                    self.set_status("READY", false);
                }
            }
            "status" => self.set_status(&text, false),
            "error" => {
                self.set_status(&text, true);
                self.play_disabled = false;
                self.game_running = false;
                self.server_on = false;
            }
            "exporting" => self.export_note = ("Exporting the logs...".into(), false),
            "exported" => {
                let file = text.rsplit(['/', '\\']).next().unwrap_or("");
                self.export_note = (trf("Saved {}: attach it to your bug report.", &[&file]), false);
            }
            "export-error" => self.export_note = (text, true),
            "exited" => {
                let ok = text == "0";
                self.set_status(&if ok { "READY".into() } else { trf("The game closed (code {})", &[&text]) }, !ok);
                self.play_disabled = false;
                self.server_on = false;
                self.game_running = false;
                send("info", "");
                self.saves_call(json!({"op": "list"}), Tag::List);
            }
            "saves" => self.saves_reply(&text),
            _ => {}
        }
    }

    fn runner_note(&mut self, info: &Value) {
        let mut found: Vec<String> = Vec::new();
        if info["umu"].as_bool() == Some(true) {
            found.push("umu-run".into());
        }
        found.extend(info["protons"].as_array().into_iter().flatten().take(4).filter_map(|p| p.as_str().map(String::from)));
        if info["wine"].as_bool() == Some(true) {
            found.push("Wine".into());
        }
        let runner = info["runner"].as_str().unwrap_or("");
        self.runner_note = if runner.is_empty() {
            "No Wine or Proton found: install Steam's Proton, umu-launcher or Wine, then reopen the launcher.".into()
        } else {
            trf("The game runs in {}, in its own prefix ({}). Found: {}.", &[&runner, &info["prefix"].as_str().unwrap_or(""), &found.join(", ")])
        };
    }

    fn auto_for_gpu(&mut self, info: &Value) {
        let Some(gpus) = info["gpus"].as_array().filter(|g| !g.is_empty()) else { return };
        if self.gpu_checked || !self.loaded {
            return;
        }
        self.gpu_checked = true;
        let by = |v: u64| gpus.iter().filter(|g| g["vendor"].as_u64() == Some(v)).max_by_key(|g| g["vram"].as_u64().unwrap_or(0));
        let (nv, amd, intel) = (by(0x10de), by(0x1002), by(0x8086));
        let discrete = |g: Option<&Value>| g.is_some_and(|g| g["vram"].as_u64().unwrap_or(0) >= 2048);
        let pick = if let Some(g) = nv { Some(("nvidia", g, true)) }
            else if discrete(amd) { Some(("amd", amd.unwrap(), true)) }
            else if discrete(intel) { Some(("intel", intel.unwrap(), true)) }
            else if let Some(g) = amd { Some(("amd", g, false)) }
            else { intel.map(|g| ("intel", g, false)) };
        let Some((adapter, gpu, disc)) = pick else { return };
        let name = gpu["name"].as_str().unwrap_or("").to_string();
        let current = self.get("graphics.adapter");
        let vendor = |a: &str| match a { "nvidia" => 0x10de, "amd" => 0x1002, "intel" => 0x8086, _ => 0 };
        let present = current.is_empty() || gpus.iter().any(|g| g["vendor"].as_u64() == Some(vendor(&current)));
        let note = if self.fresh {
            self.set("graphics.adapter", json!(adapter));
            self.set("graphics.quality", json!(if disc { "1" } else { "3" }));
            self.set("performance.fps", json!(if disc { "60" } else { "30" }));
            trf("Set for {}: {}. Change it in Settings.",
                &[&name, &tr(if disc { "Best quality at 60 fps" } else { "Medium quality at 30 fps (integrated graphics)" })])
        } else if !present {
            self.set("graphics.adapter", json!(adapter));
            trf("GPU set to {}: the card chosen before is not in this PC.", &[&name])
        } else {
            return;
        };
        self.set_status(&note, false);
    }

    fn setup_line(&mut self, line: &str) {
        let (kind, text) = line.split_once(':').unwrap_or((line, ""));
        match kind {
            "step" => {
                let (frac, what) = text.split_once(':').unwrap_or((text, ""));
                let (i, n) = frac.split_once('/').unwrap_or(("0", "1"));
                let (i, n) = (i.parse::<f32>().unwrap_or(0.0), n.parse::<f32>().unwrap_or(1.0).max(1.0));
                self.setup_step = ((i - 1.0).max(0.0), n);
                self.setup_text = tr(what).to_string();
                self.setup_bar = (i - 1.0).max(0.0) / n;
            }
            "progress" => {
                let (i, n) = self.setup_step;
                self.setup_bar = (i + text.parse::<f32>().unwrap_or(0.0) / 100.0) / n;
            }
            "note" => self.setup_text = format!("{} · {}", self.setup_text, tr(text)),
            "error" => self.setup_error = text.to_string(),
            _ => {}
        }
    }

    fn saves_call(&mut self, req: Value, tag: Tag) {
        self.queue.push_back((req, tag));
        self.saves_next();
    }
    fn saves_next(&mut self) {
        if self.saves_busy {
            return;
        }
        if let Some((req, _)) = self.queue.front() {
            self.saves_busy = true;
            send("saves", &req.to_string());
        }
    }
    fn saves_msg(&mut self, text: &str, error: bool) {
        self.saves_msg = (text.to_string(), error);
    }
    fn saves_do(&mut self, req: Value, done: Done, after: After) {
        if self.game_running || self.saves_running {
            self.saves_msg("The game is running. Close it before changing saves.", true);
            return;
        }
        self.saves_msg("Working...", false);
        self.saves_call(req, Tag::Do { done, after });
    }
    fn saves_reply(&mut self, text: &str) {
        let r: Value = serde_json::from_str(text).unwrap_or(json!({"ok": false, "error": "The save tool gave no answer."}));
        let tag = self.queue.pop_front().map(|(_, t)| t);
        self.saves_busy = false;
        if let Some(slots) = r["slots"].as_array() {
            self.slots = slots.clone();
            self.active = r["active"].as_str().unwrap_or("").to_string();
            self.saves_running = r["running"].as_bool().unwrap_or(false);
            if self.saves_running {
                self.saves_msg("The game is running: saves can be changed once it is closed.", true);
            }
            if let Some(a) = self.slots.iter().find(|s| s["active"].as_bool() == Some(true)) {
                let started = a["started"].as_bool().unwrap_or(false);
                let player = a["player"].as_str().unwrap_or("");
                self.save_label = format!("{}{}", a["name"].as_str().unwrap_or(""),
                    if !started { tr(" · New game").into() } else if !player.is_empty() { trf(" · {} Lv {}", &[&player, &a["level"]]) } else { String::new() });
            }
        }
        let ok = r["ok"].as_bool() == Some(true);
        match tag {
            Some(Tag::Do { done, after }) => {
                if !ok {
                    self.saves_msg(r["error"].as_str().unwrap_or("That did not work."), true);
                } else {
                    let msg = match done {
                        Done::Text(t) => t,
                        Done::Delete(n) => trf("\"{}\" was deleted.{}", &[&n, &r["backup"].as_str().map(|b| trf(" Its copy: {}.", &[&b])).unwrap_or_default()]),
                        Done::Backup(n) => trf("A copy of \"{}\" was kept: {}.", &[&n, &r["backup"].as_str().unwrap_or("")]),
                        Done::Apply(n) => trf("Applied to \"{}\": {} new, {} leveled, {} amounts set. A copy from before: {}.",
                                              &[&n, &r["result"]["granted"], &r["result"]["leveled"], &r["result"]["items"], &r["backup"].as_str().unwrap_or("")]),
                    };
                    self.saves_msg(&msg, false);
                    match after {
                        After::None => {}
                        After::CloseEditor => self.close_editor(),
                        After::Editor(id) => self.open_editor(id, &r),
                        After::Applied => {
                            if let Some(ed) = &mut self.ed {
                                ed.player = r["player"].clone();
                            }
                            self.pending = Pending::default();
                            self.edits.clear();
                            self.amount_edits.clear();
                            self.saves_call(json!({"op": "list"}), Tag::List);
                        }
                    }
                }
            }
            Some(Tag::Images(keys, target)) => {
                if ok {
                    for k in r["have"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        self.pic_have.insert(k.to_string());
                    }
                } else {
                    for k in keys {
                        self.pic_asked.remove(&k);
                    }
                }
                match target {
                    PicsFor::Editor => self.saves_msg("", false),
                    PicsFor::Events => self.ct_msg = (self.ct_summary(), false),
                }
            }
            Some(Tag::ContentList) => {
                if !ok {
                    self.ct_msg = (r["error"].as_str().unwrap_or("The list could not be read.").into(), true);
                } else {
                    self.ct_loaded = true;
                    self.ct_banners = r["banners"].as_array().cloned().unwrap_or_default();
                    self.ct_events = r["events"].as_array().cloned().unwrap_or_default();
                    let keys: Vec<String> = self.ct_banners.iter().chain(&self.ct_events)
                        .filter_map(|x| x["img"].as_str().map(String::from)).collect();
                    self.ct_msg = (self.ct_summary(), false);
                    self.want_pics(keys, PicsFor::Events);
                }
            }
            Some(Tag::ContentSet) => {
                self.ct_msg = if ok {
                    (trf("Saved. {}{}", &[&self.ct_summary(), &if self.game_running { tr(" Events change the next time the game starts.") } else { "" }]), false)
                } else {
                    (r["error"].as_str().unwrap_or("The choice could not be saved.").into(), true)
                };
            }
            Some(Tag::List) | None => {}
        }
        self.saves_next();
    }
    fn want_pics(&mut self, keys: Vec<String>, target: PicsFor) {
        let mut todo: Vec<String> = keys.into_iter().filter(|k| !k.is_empty() && !self.pic_asked.contains(k)).collect();
        todo.sort();
        todo.dedup();
        if todo.is_empty() {
            return;
        }
        let msg = "Making the pictures from your game files (the first time takes a few seconds)...";
        match target {
            PicsFor::Editor => self.saves_msg(msg, false),
            PicsFor::Events => self.ct_msg = (msg.into(), false),
        }
        for k in &todo {
            self.pic_asked.insert(k.clone());
        }
        self.saves_call(json!({"op": "images", "keys": todo}), Tag::Images(todo, target));
    }
    fn slot_name(&self, id: &str) -> String {
        self.slots.iter().find(|s| s["id"].as_str() == Some(id)).and_then(|s| s["name"].as_str()).unwrap_or(tr("this save")).to_string()
    }
    fn ask(&mut self, title: &str, text: &str, input: Option<&str>, ok: &str, ask: Ask) {
        self.modal = Some(Modal { title: title.into(), text: text.into(), input: input.map(String::from), ok: ok.into(), ask, focus: true });
    }
    fn keep_pending(&mut self, then: Then) {
        if self.pending.count() == 0 {
            return self.then(then);
        }
        self.ask("Changes not applied", "The changes made in the editor are not applied yet. Drop them?", None, "Drop them", Ask::Drop(then));
    }
    fn then(&mut self, then: Then) {
        match then {
            Then::Use(id) => {
                let (name, cur) = (self.slot_name(&id), self.slot_name(&self.active.clone()));
                self.ask("Use this save", &trf("The game will start with \"{}\" next time. \"{}\" is kept as it is and can be used again.", &[&name, &cur]),
                         None, "Use it", Ask::Use(id));
            }
            Then::Edit(id) => self.saves_do(json!({"op": "read", "id": id}), Done::Text(String::new()), After::Editor(id)),
            Then::Restart => {
                let n = self.slots.len() + 1;
                let cur = self.slot_name(&self.active.clone());
                self.ask("Start over", &trf("A new save is made and used: the game starts from the beginning next time. \"{}\" is kept as it is and can be used again from this list. Name of the new save:", &[&cur]),
                         Some(&trf("New game {}", &[&n])), "Start over", Ask::Restart);
            }
        }
    }
    fn answer(&mut self, ask: Ask, input: Option<String>) {
        match ask {
            Ask::Use(id) => {
                let name = self.slot_name(&id);
                self.saves_do(json!({"op": "switch", "id": id}), Done::Text(trf("\"{}\" is in use. Press START to play it.", &[&name])), After::CloseEditor);
            }
            Ask::Rename(id) => {
                if let Some(to) = input {
                    self.saves_do(json!({"op": "rename", "id": id, "name": to}), Done::Text("Renamed.".into()), After::None);
                }
            }
            Ask::Delete(id) => {
                let name = self.slot_name(&id);
                if self.ed.as_ref().is_some_and(|e| e.id == id) {
                    self.close_editor();
                }
                self.saves_do(json!({"op": "delete", "id": id}), Done::Delete(name), After::None);
            }
            Ask::Restart => {
                if let Some(name) = input {
                    self.saves_do(json!({"op": "new", "name": name, "switch": true}),
                                  Done::Text(trf("\"{}\" is in use. Press START to begin a new game.", &[&name])), After::CloseEditor);
                }
            }
            Ask::NewEmpty => {
                if let Some(name) = input {
                    self.saves_do(json!({"op": "new", "name": name, "switch": false}),
                                  Done::Text(trf("\"{}\" was made. Choose Use on it to play it.", &[&name])), After::None);
                }
            }
            Ask::PhoneImport(path) => {
                if let Some(name) = input {
                    self.saves_do(json!({"op": "import", "path": path, "name": name}),
                                  Done::Text(trf("\"{}\" was imported. Choose Use on it to play it.", &[&name])), After::None);
                }
            }
            Ask::SetAllShown => {
                let amount = input.and_then(|v| v.trim().parse::<f64>().ok()).filter(|a| *a >= 0.0);
                if let Some(a) = amount {
                    let ids: Vec<i64> = self.ed_items().iter().map(|x| x.id).collect();
                    let kind = self.ed_kind;
                    for id in ids {
                        self.pending.map(kind).insert(id, (a.floor() as i64).min(2147483647));
                        self.edits.remove(&(kind.to_string(), id));
                    }
                }
            }
            Ask::Drop(then) => {
                self.pending = Pending::default();
                self.edits.clear();
                self.amount_edits.clear();
                self.then(then);
            }
            Ask::Apply => self.apply(),
            Ask::PlayAnyway => self.play(),
        }
    }
    fn close_editor(&mut self) {
        self.ed = None;
        self.pending = Pending::default();
        self.edits.clear();
        self.amount_edits.clear();
    }
    fn open_editor(&mut self, id: String, r: &Value) {
        let mut catalog = r["catalog"].clone();
        let mut char_name = HashMap::new();
        for c in catalog["characters"].as_array().into_iter().flatten() {
            char_name.insert(c["id"].as_i64().unwrap_or(0), c["name"].as_str().unwrap_or("").to_string());
        }
        for k in ["costumes", "weapons"] {
            if let Some(list) = catalog[k].as_array_mut() {
                list.sort_by(|a, b| {
                    let cn = |x: &Value| char_name.get(&x["char"].as_i64().unwrap_or(-1)).cloned().unwrap_or_default();
                    b["rarity"].as_i64().unwrap_or(0).cmp(&a["rarity"].as_i64().unwrap_or(0))
                        .then_with(|| cn(a).cmp(&cn(b)))
                        .then_with(|| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")))
                });
            }
        }
        if let Some(list) = catalog["companions"].as_array_mut() {
            list.sort_by(|a, b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
        }
        let keys: Vec<String> = ED_KINDS.iter().flat_map(|(k, _)| catalog[*k].as_array().cloned().unwrap_or_default())
            .filter_map(|x| x["img"].as_str().map(String::from)).collect();
        self.ed = Some(Editor { id, player: r["player"].clone(), catalog, char_name });
        self.pending = Pending::default();
        self.edits.clear();
        self.amount_edits.clear();
        self.saves_msg("", false);
        self.want_pics(keys, PicsFor::Editor);
    }
    fn ed_items(&self) -> Vec<Item> {
        let Some(ed) = &self.ed else { return Vec::new() };
        let (c, p) = (&ed.catalog, &ed.player);
        let q = self.ed_search.trim().to_lowercase();
        let kind = self.ed_kind;
        let levels = LEVEL_KINDS.contains(&kind);
        c[kind].as_array().into_iter().flatten().map(|x| {
            let id = x["id"].as_i64().unwrap_or(0);
            let have = if levels {
                p[kind].get(id.to_string()).filter(|v| !v.is_null()).cloned()
            } else {
                p[kind].get(id.to_string()).and_then(Value::as_i64).filter(|n| *n != 0).map(|n| json!({"count": n}))
            };
            Item {
                id,
                name: x["name"].as_str().unwrap_or("").to_string(),
                sub: if kind == "costumes" { ed.char_name.get(&x["char"].as_i64().unwrap_or(-1)).cloned().unwrap_or_default() } else { String::new() },
                img: x["img"].as_str().unwrap_or("").to_string(),
                rarity: x["rarity"].as_i64().unwrap_or(0),
                have,
            }
        }).filter(|x| {
            (q.is_empty() || x.name.to_lowercase().contains(&q) || x.sub.to_lowercase().contains(&q) || x.id.to_string() == q)
                && (self.ed_rarity.is_empty() || !levels || kind == "companions" || x.rarity.to_string() == self.ed_rarity)
                && (self.ed_owned.is_empty() || (self.ed_owned == "yes") == x.have.is_some())
        }).collect()
    }
    fn pending_text(&self) -> String {
        let mut parts = Vec::new();
        for (kind, label) in [("costumes", "Costumes"), ("weapons", "Weapons"), ("companions", "Companions")] {
            let Some(map) = self.pending.get(kind).filter(|m| !m.is_empty()) else { continue };
            let owned = self.ed.as_ref().map(|e| e.player[kind].clone()).unwrap_or(Value::Null);
            let add = map.keys().filter(|id| owned.get(id.to_string()).is_none_or(Value::is_null)).count();
            let level = map.values().filter(|l| **l != 0).count();
            let mut bits = Vec::new();
            if add > 0 {
                bits.push(trf("{} to own", &[&add]));
            }
            if level > 0 {
                bits.push(trf("{} to level", &[&level]));
            }
            parts.push(trf("{}: {}", &[&tr(label), &bits.join(", ")]));
        }
        for (kind, label) in [("materials", "Materials"), ("consumables", "Items")] {
            if let Some(m) = self.pending.get(kind).filter(|m| !m.is_empty()) {
                parts.push(trf("{}: {}", &[&tr(label), &m.len()]));
            }
        }
        for (k, v) in &self.pending.amounts {
            let name = match k.as_str() { "freeGems" => "Gems", "paidGems" => "Paid gems", _ => "Gold" };
            parts.push(trf("{}: {}", &[&tr(name), &thousands(*v)]));
        }
        parts.join(" · ")
    }
    fn apply(&mut self) {
        let Some(ed) = &self.ed else { return };
        let (id, name) = (ed.id.clone(), self.slot_name(&ed.id.clone()));
        let list = |m: Option<&BTreeMap<i64, i64>>| -> Value {
            Value::Array(m.into_iter().flatten().map(|(id, l)| json!({"id": id, "level": l})).collect())
        };
        let obj = |m: Option<&BTreeMap<i64, i64>>| -> Value {
            Value::Object(m.into_iter().flatten().map(|(id, n)| (id.to_string(), json!(n))).collect())
        };
        let mut edit = json!({
            "costumes": list(self.pending.get("costumes")), "weapons": list(self.pending.get("weapons")),
            "companions": list(self.pending.get("companions")),
            "materials": obj(self.pending.get("materials")), "consumables": obj(self.pending.get("consumables")),
        });
        for (k, v) in &self.pending.amounts {
            edit[k] = json!(v);
        }
        self.saves_do(json!({"op": "apply", "id": id, "edit": edit}), Done::Apply(name), After::Applied);
    }

    fn ct_summary(&self) -> String {
        let on = |l: &[Value]| l.iter().filter(|x| x["on"].as_bool() == Some(true)).count();
        trf("Open: {} of {} summons, {} of {} events.", &[&on(&self.ct_banners), &self.ct_banners.len(), &on(&self.ct_events), &self.ct_events.len()])
    }
    fn ct_load(&mut self) {
        self.ct_msg = ("Reading the summons and events...".into(), false);
        self.saves_call(json!({"op": "content-list"}), Tag::ContentList);
    }
    fn ct_save(&mut self) {
        self.ct_msg = ("Saving...".into(), false);
        self.ct_save_at = Some(Instant::now() + Duration::from_millis(600));
    }

    fn play(&mut self) {
        self.play_disabled = true;
        self.game_running = true;
        self.server_on = true;
        self.set_status("STARTING", false);
        st::free_toggle(&mut self.st);
        self.save();
        self.flush_save();
        let mut lines = Vec::new();
        if self.st["controls"]["enabled"].as_bool() == Some(true) {
            lines.push(format!("input:{}", self.st["controls"]));
        }
        lines.push(format!("mods:{}", st::mods_file(&self.st)));
        for (k, v) in st::launch_prefs(&self.st) {
            lines.push(format!("pref:{k}={v}"));
        }
        for (k, v) in st::launch_env(&self.st) {
            lines.push(format!("env:{k}={v}"));
        }
        send("play", &lines.join("\n"));
    }

    fn go(&mut self, view: View) {
        self.view = view;
        if view == View::Events && !self.ct_loaded {
            self.ct_load();
        }
    }
}

fn thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let mut first = Vec::new();
    if let Ok(bytes) = std::fs::read(crate::path("launcher/ui/game/Rodin.otf")) {
        let tweak = egui::FontTweak { y_offset_factor: 0.45, ..Default::default() };
        defs.font_data.insert("rodin".into(), egui::FontData::from_owned(bytes).tweak(tweak).into());
        first.push("rodin".to_string());
    } else {
        for f in ["/usr/share/fonts/noto/NotoSans-Regular.ttf", "/usr/share/fonts/TTF/DejaVuSans.ttf"] {
            if let Ok(bytes) = std::fs::read(f) {
                defs.font_data.insert("sans".into(), egui::FontData::from_owned(bytes).into());
                first.push("sans".to_string());
                break;
            }
        }
    }
    let mut last = Vec::new();
    for f in ["/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", "/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc",
              "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc"] {
        if let Ok(bytes) = std::fs::read(f) {
            defs.font_data.insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            last.push("cjk".to_string());
            break;
        }
    }
    let fam = defs.families.entry(egui::FontFamily::Proportional).or_default();
    for (i, f) in first.iter().enumerate() {
        fam.insert(i, f.clone());
    }
    fam.extend(last.clone());
    defs.families.entry(egui::FontFamily::Monospace).or_default().extend(last);
    ctx.set_fonts(defs);
}

fn vk(key: egui::Key) -> Option<(u32, String)> {
    use egui::Key::*;
    let name = key.name().to_string();
    let code = match key {
        A | B | C | D | E | F | G | H | I | J | K | L | M | N | O | P | Q | R | S | T | U | V | W | X | Y | Z => {
            name.chars().next()? as u32
        }
        Num0 | Num1 | Num2 | Num3 | Num4 | Num5 | Num6 | Num7 | Num8 | Num9 => {
            return Some((name.chars().next()? as u32, name));
        }
        Space => return Some((32, "Space".into())),
        Enter => return Some((13, "Enter".into())),
        Backspace => return Some((8, "Backspace".into())),
        Tab => return Some((9, "Tab".into())),
        ArrowLeft => return Some((37, "ArrowLeft".into())),
        ArrowUp => return Some((38, "ArrowUp".into())),
        ArrowRight => return Some((39, "ArrowRight".into())),
        ArrowDown => return Some((40, "ArrowDown".into())),
        Insert => return Some((45, "Insert".into())),
        Delete => return Some((46, "Delete".into())),
        Home => return Some((36, "Home".into())),
        End => return Some((35, "End".into())),
        PageUp => return Some((33, "PageUp".into())),
        PageDown => return Some((34, "PageDown".into())),
        Comma => return Some((188, ",".into())),
        Period => return Some((190, ".".into())),
        Minus => return Some((189, "-".into())),
        Semicolon => return Some((186, ";".into())),
        Slash => return Some((191, "/".into())),
        F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 => {
            let n: u32 = name[1..].parse().ok()?;
            return Some((111 + n, name));
        }
        _ => return None,
    };
    Some((code, name))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        while let Ok(m) = self.rx.try_recv() {
            self.on_message(&ctx, m);
        }
        self.pics.poll(&ctx);
        if self.save_at.is_some_and(|t| Instant::now() >= t) {
            self.flush_save();
        }
        if self.ct_save_at.is_some_and(|t| Instant::now() >= t) {
            self.ct_save_at = None;
            let banners: Vec<Value> = self.ct_banners.iter().filter(|x| x["on"].as_bool() == Some(true) && x["always"].as_bool() != Some(true))
                .map(|x| x["key"].clone()).collect();
            let events: Vec<Value> = self.ct_events.iter().filter(|x| x["on"].as_bool() == Some(true)).map(|x| x["id"].clone()).collect();
            self.saves_call(json!({"op": "content-set", "banners": banners, "events": events}), Tag::ContentSet);
        }
        if self.save_at.is_some() || self.ct_save_at.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some(i) = self.listening {
            let pressed = ctx.input(|inp| inp.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, .. } => Some(*key),
                _ => None,
            }));
            if let Some(key) = pressed {
                self.listening = None;
                if key != egui::Key::Escape {
                    if let Some((code, name)) = vk(key) {
                        self.st["controls"]["binds"][i]["vk"] = json!(code);
                        self.st["controls"]["binds"][i]["key"] = json!(name);
                        self.save();
                    }
                }
            }
        }

        let full = ui.max_rect();
        if self.ready == Some(false) {
            self.setup_page(ui, full);
        } else if self.view == View::Home {
            self.title_page(ui, full);
        } else {
            self.menu_page(ui, full);
        }
        self.modal_ui(&ctx);
        if self.saves_busy {
            ctx.set_cursor_icon(egui::CursorIcon::Progress);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_save();
        crate::message("quit");
    }
}

impl App {
    fn setup_page(&mut self, ui: &mut Ui, full: Rect) {
        let p = ui.painter();
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(full.center(), Color32::from_rgb(0x4a, 0x44, 0x40));
        let n = 48;
        for i in 0..=n {
            let t = i as f32 / n as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(full.center() + vec2(t.cos() * full.width() * 0.75, t.sin() * full.height() * 0.75), Color32::from_rgb(0x2a, 0x26, 0x24));
        }
        for i in 1..=n as u32 {
            mesh.add_triangle(0, i, i + 1);
        }
        p.add(mesh);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let w = 860.0_f32.min(full.width() * 0.92);
            ui.add_space(((full.height() - 640.0) / 2.0).max(30.0));
            ui.horizontal(|ui| {
                ui.add_space((full.width() - w) / 2.0);
                ui.vertical(|ui| {
                    ui.set_width(w);
                    dialog(ui, "Prepare the game", |ui| {
                        lead(ui, "This port ships without the game. Give it your own copy of NieR Re[in]carnation (Global 3.7.1): it is checked, prepared for the PC and installed here. Your files are not changed.");
                        type Picks = [(&'static str, &'static str, bool)];
                        let rows: [(&str, &str, &Picks); 3] = [
                            ("1 · The game (APK)", "com.square_enix.android_googleplay.nierspww 3.7.1, arm64 .apk", &[("Choose", "apk", false)]),
                            ("2 · The game's assets", "resource_dump_android.7z or .zip (about 15 GB; 21 GB once extracted)",
                             &[("Archive", "assets", false), ("Folder", "assets-folder", true)]),
                            ("3 · The master data", "20240404193219.bin.e", &[("Choose", "master", false)]),
                        ];
                        for (i, (title, hint, picks)) in rows.iter().enumerate() {
                            bar(ui, title);
                            ui.horizontal(|ui| {
                                let chosen = !self.choice[i].is_empty();
                                let text = if chosen { self.choice[i].as_str() } else { tr(hint) };
                                let bw: f32 = picks.iter().map(|p| 148.0_f32.max(s::spaced(ui, tr(p.0), 17.0, INK, 0.0).size().x + 98.0)).sum();
                                let g = s::wrapped(ui, text, 14.0, if chosen { INK } else { DIM }, ui.available_width() - bw - 14.0);
                                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - bw, g.size().y.max(40.0)), Sense::hover());
                                ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, INK);
                                for (label, kind, dim) in picks.iter() {
                                    if hex(ui, label, *dim, true, true).clicked() {
                                        send("pick", kind);
                                    }
                                }
                            });
                        }
                        progress(ui, self.setup_bar);
                        lead(ui, &self.setup_text.clone());
                        lead_colored(ui, &self.setup_error.clone(), ALERT);
                        let can = !self.setup_running && self.choice.iter().all(|c| !c.is_empty());
                        if buttons(ui, &[("Prepare", false, can)]) == Some(0) {
                            self.setup_running = true;
                            self.setup_error.clear();
                            send("setup", &self.choice.join("\n"));
                        }
                    });
                });
            });
        });
        if self.setup_running {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }

    fn title_page(&mut self, ui: &mut Ui, full: Rect) {
        let p = ui.painter().clone();
        p.rect_filled(full, 0.0, Color32::from_rgb(0xec, 0xe8, 0xe0));
        if let Some(t) = self.pics.get("launcher/ui/game/title.jpg") {
            let (tw, th) = (t.size()[0] as f32, t.size()[1] as f32);
            let scale = (full.width() / tw).max(full.height() / th);
            let (w, h) = (tw * scale, th * scale);
            let r = Rect::from_min_size(pos2(full.center().x - w / 2.0, full.top() + (full.height() - h) * 0.4), vec2(w, h));
            p.image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let mut mesh = egui::Mesh::default();
        let ys = [(0.76, 0.0), (0.90, 0.5), (1.0, 0.82)];
        for (y, a) in ys {
            let c = rgba(236, 232, 224, a);
            mesh.colored_vertex(pos2(full.left(), full.top() + y * full.height()), c);
            mesh.colored_vertex(pos2(full.right(), full.top() + y * full.height()), c);
        }
        for i in 0..2u32 {
            mesh.add_triangle(i * 2, i * 2 + 1, i * 2 + 3);
            mesh.add_triangle(i * 2, i * 2 + 3, i * 2 + 2);
        }
        p.add(mesh);

        if hexicon(ui, Rect::from_min_size(pos2(full.right() - 64.0 - 70.0, full.top() + 32.0), vec2(70.0, 44.0))).clicked() {
            self.go(View::Settings);
        }

        let ink = Color32::from_rgb(0x1f, 0x1c, 0x1a);
        p.text(pos2(full.center().x, full.bottom() - 24.0), Align2::CENTER_CENTER,
               tr("An unofficial fan port · NieR Re[in]carnation © SQUARE ENIX · not affiliated with Square Enix"), font(14.0), Color32::from_rgb(0x3f, 0x3a, 0x36));
        let record: Vec<(&str, String, Color32)> = vec![
            ("Game files", self.files.0.clone(), match self.files.1 { Some(false) => Color32::from_rgb(0x9b, 0x2f, 0x28), Some(true) => Color32::from_rgb(0x2c, 0x28, 0x25), None => ink }),
            ("Lunar Tear", if self.server_on { "Running".into() } else { "Standing by".into() }, if self.server_on { Color32::from_rgb(0x2c, 0x28, 0x25) } else { ink }),
            ("Save", self.save_label.clone(), ink),
            ("Settings", st::summary(&self.st), ink),
        ];
        let galleys: Vec<_> = record.iter().map(|(dt, dd, c)| {
            (s::spaced(ui, tr(dt), 17.0, Color32::from_rgb(0x57, 0x51, 0x4b), 0.0), s::spaced(ui, tr(dd), 17.0, *c, 0.0))
        }).collect();
        let total: f32 = galleys.iter().map(|(a, b)| a.size().x + 8.0 + b.size().x).sum::<f32>() + 34.0 * 3.0;
        let mut x = full.center().x - total / 2.0;
        let y = full.bottom() - 50.0 - 26.0;
        for (a, b) in galleys {
            let (aw, bw) = (a.size().x, b.size().x);
            p.galley(pos2(x, y), a, INK);
            p.galley(pos2(x + aw + 8.0, y), b, INK);
            x += aw + 8.0 + bw + 34.0;
        }
        let mut y = y - 38.0;
        let sc = if self.status_error { Color32::from_rgb(0x9b, 0x2f, 0x28) } else { Color32::from_rgb(0x2f, 0x2b, 0x28) };
        let status = |c: Color32| {
            let mut job = egui::text::LayoutJob::default();
            job.append(&tr(&self.status).to_uppercase(), 0.0, egui::TextFormat { font_id: font(14.0), color: c, extra_letter_spacing: 2.0, ..Default::default() });
            job.wrap.max_width = 1240.0_f32.min(full.width() * 0.82);
            job.halign = egui::Align::Center;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let g = status(sc);
        y -= g.size().y - 20.0;
        let at = pos2(full.center().x, y);
        p.galley(at + vec2(0.0, 1.0), status(rgba(255, 255, 255, 0.6)), sc);
        p.galley(at, g, sc);

        let cy = y - 34.0;
        let w = 1240.0_f32.min(full.width() * 0.82);
        let start = s::spaced(ui, "START", 27.0, Color32::BLACK, 4.3);
        let sw = start.size().x + 140.0;
        let rule = |x0: f32, x1: f32, fade_left: bool| {
            let mut m = egui::Mesh::default();
            let (c0, c1) = if fade_left { (Color32::TRANSPARENT, rgba(40, 36, 33, 0.55)) } else { (rgba(40, 36, 33, 0.55), Color32::TRANSPARENT) };
            m.colored_vertex(pos2(x0, cy), c0);
            m.colored_vertex(pos2(x1, cy), c1);
            m.colored_vertex(pos2(x1, cy + 1.0), c1);
            m.colored_vertex(pos2(x0, cy + 1.0), c0);
            m.add_triangle(0, 1, 2);
            m.add_triangle(0, 2, 3);
            p.add(m);
        };
        rule(full.center().x - w / 2.0, full.center().x - sw / 2.0, true);
        rule(full.center().x + sw / 2.0, full.center().x + w / 2.0, false);
        let srect = Rect::from_center_size(pos2(full.center().x, cy), vec2(sw, 40.0));
        let enabled = !self.play_disabled;
        let resp = ui.interact(srect, ui.scope_id().with("start"), if enabled { Sense::click() } else { Sense::hover() });
        let t = self.started.elapsed().as_secs_f32();
        let breathe = 0.725 + 0.275 * (t / 2.6 * std::f32::consts::TAU).cos();
        let a = if !enabled { 0.45 } else if resp.hovered() { 1.0 } else { breathe };
        let glow = s::spaced(ui, "START", 27.0, rgba(255, 255, 255, 0.55 * a), 4.3);
        let pos = srect.center() - start.size() / 2.0;
        for d in [vec2(-1.5, 0.0), vec2(1.5, 0.0), vec2(0.0, -1.5), vec2(0.0, 1.5)] {
            p.galley(pos + d, glow.clone(), INK);
        }
        let text = s::spaced(ui, "START", 27.0, rgba(0x1a, 0x17, 0x15, a), 4.3);
        p.galley(pos, text, INK);
        if enabled {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            if self.pending.count() > 0 {
                self.ask("Start the game", "The changes in Settings › Saves are not applied yet. Start the game without them?", None, "Start the game", Ask::PlayAnyway);
            } else {
                self.play();
            }
        }
    }

    fn menu_page(&mut self, ui: &mut Ui, full: Rect) {
        page_background(ui.painter(), full);
        let name = match self.view { View::Events => "Events", View::Controls => "Controls", _ => "Settings" };
        let top = Rect::from_min_size(full.min, vec2(full.width(), 82.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(top.shrink2(vec2(60.0, 0.0)).translate(vec2(0.0, 10.0))), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 26.0;
                if square(ui, "←").clicked() {
                    self.go(View::Home);
                }
                let g = s::spaced(ui, tr(name), 34.0, INK, 6.8);
                let (r, _) = ui.allocate_exact_size(vec2(g.size().x, 62.0), Sense::hover());
                ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, INK);
                ui.add_space(ui.available_width() - 62.0);
                if square(ui, "✕").clicked() {
                    self.flush_save();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
        let tabs_h = 84.0;
        let bottom = Rect::from_min_max(pos2(full.left(), full.bottom() - tabs_h), full.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
            let list = [("Title", View::Home), ("Settings", View::Settings), ("Events", View::Events), ("Controls", View::Controls)];
            let total = 4.0 * 210.0 + 3.0 * 18.0;
            ui.horizontal(|ui| {
                ui.add_space(((full.width() - total) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing.x = 18.0;
                ui.add_space(0.0);
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 18.0;
                        for (label, v) in list {
                            if tab(ui, label, self.view == v, 210.0, 52.0, 19.0, 6.0).clicked() {
                                self.go(v);
                            }
                        }
                    });
                });
            });
        });
        let main = Rect::from_min_max(pos2(full.left(), top.bottom()), pos2(full.right(), bottom.top()));
        ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| {
            egui::ScrollArea::vertical().id_salt(("main", self.view as u8, self.sub)).auto_shrink([false, false]).show(ui, |ui| {
                let w = (ui.available_width() - 120.0).min(1280.0);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - w) / 2.0);
                    ui.vertical(|ui| {
                        ui.set_width(w);
                        ui.add_space(14.0);
                        match self.view {
                            View::Settings => self.settings_page(ui),
                            View::Events => self.events_page(ui),
                            View::Controls => self.controls_page(ui),
                            View::Home => {}
                        }
                    });
                });
            });
        });
    }

    fn sub_tabs(&mut self, ui: &mut Ui) {
        let w = ui.available_width();
        let total = SUBS.iter().map(|(_, l)| 132.0_f32.max(s::spaced(ui, tr(l), 17.0, INK, 0.6).size().x + 40.0) + 10.0).sum::<f32>() - 10.0;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            ui.add_space(((w - total) / 2.0).max(0.0));
            for (key, label) in SUBS {
                if tab(ui, label, self.sub == key, 132.0, 42.0, 17.0, 5.0).clicked() {
                    self.sub = key;
                    if key == "saves" {
                        self.saves_call(json!({"op": "list"}), Tag::List);
                    }
                }
            }
        });
        ui.add_space(14.0);
    }

    fn cols(&mut self, ui: &mut Ui, left: impl FnOnce(&mut Self, &mut Ui), right: impl FnOnce(&mut Self, &mut Ui)) {
        let w = ui.available_width();
        let col = (w - 40.0) / 2.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 40.0;
            ui.vertical(|ui| {
                ui.set_width(col);
                left(self, ui);
            });
            ui.vertical(|ui| {
                ui.set_width(col);
                right(self, ui);
            });
        });
    }
    fn one(&mut self, ui: &mut Ui, max: f32, body: impl FnOnce(&mut Self, &mut Ui)) {
        let w = ui.available_width().min(max);
        ui.horizontal_top(|ui| {
            ui.add_space((ui.available_width() - w) / 2.0);
            ui.vertical(|ui| {
                ui.set_width(w);
                body(self, ui);
            });
        });
    }
    fn sublead(&mut self, ui: &mut Ui, text: &str, error: bool) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() - 8.0);
                lead_colored(ui, text, if error { ALERT } else { INK });
            });
        });
    }

    fn settings_page(&mut self, ui: &mut Ui) {
        self.sub_tabs(ui);
        match self.sub {
            "display" => self.cols(ui, |a, ui| {
                dialog(ui, "Screen", |ui| {
                    bar(ui, "Mode");
                    a.choice(ui, "display.mode", &[("fullscreen", "Full screen"), ("window", "Window")]);
                    if a.get("display.mode") == "window" {
                        bar(ui, "Window size");
                        a.choice(ui, "display.size", &[("1280x720", "1280×720"), ("1600x900", "1600×900"), ("1920x1080", "1920×1080")]);
                    }
                });
            }, |a, ui| {
                dialog(ui, "Renderer", |ui| {
                    bar(ui, "Renderer");
                    a.choice(ui, "graphics.backend", &[("d3d11", "Direct3D 11"), ("vulkan", "Vulkan"), ("swiftshader", "Software")]);
                    lead(ui, "On Linux the game runs in Wine/Proton: Vulkan (the default) goes straight to your driver; Direct3D 11 goes through DXVK. Try the other one if the 3D looks wrong.");
                    if a.get("graphics.backend") == "vulkan" {
                        bar(ui, "Vulkan compatibility");
                        a.choice(ui, "graphics.vkCompat", &[("auto", "Auto"), ("on", "On"), ("off", "Off")]);
                        lead(ui, "For the newest graphics cards and drivers (3D black while menus show). Auto turns it on for the GeForce RTX 50 series.");
                    }
                    bar(ui, "GPU");
                    a.choice(ui, "graphics.adapter", &[("nvidia", "NVIDIA"), ("amd", "AMD"), ("intel", "Intel"), ("", "Auto")]);
                });
            }),
            "graphics" => self.cols(ui, |a, ui| {
                dialog(ui, "Quality", |ui| {
                    bar(ui, "Game quality");
                    let note = st::quality(&a.get("graphics.quality")).map_or("The game's own choice (Menu › Settings › Game Settings in the game).".to_string(),
                                                                            |q| trf("{} Applied when the game starts.", &[&tr(q.3)]));
                    lead(ui, &note);
                    a.choice(ui, "graphics.quality", &[("", "In-game"), ("6", "Max"), ("1", "Best"), ("2", "High"), ("3", "Medium"), ("4", "Low"), ("5", "Power saving")]);
                    bar(ui, "Anti-aliasing");
                    lead(ui, "FXAA smooths jagged edges over the whole picture, at almost no cost.");
                    a.choice(ui, "graphics.aa", &[("off", "Off"), ("fxaa", "FXAA")]);
                });
                dialog(ui, "Image", |ui| {
                    lead(ui, "These and the rest of this page work on their own, on top of the game quality. Game default keeps the game as it is.");
                    bar(ui, "Render resolution");
                    a.choice(ui, "custom.renderScale", &[("", "Game default"), ("100", "100%"), ("85", "85%"), ("75", "75%"), ("67", "67%"), ("50", "50%")]);
                    lead(ui, "The game draws at this share of the window and the picture is scaled up. Lower is faster on a weak graphics card; 100% is sharp even when the game quality asks for less.");
                    lead_colored(ui, "Not applied on Linux for now: in Wine it left the game's picture in a corner of the screen.", ALERT);
                    bar(ui, "Upscaling");
                    a.choice(ui, "custom.upscale", &[("", "Plain"), ("fsr", "FSR 1"), ("sgsr", "SGSR")]);
                    lead(ui, "How a picture drawn below the window's resolution is enlarged. FSR 1 is the sharpest, SGSR is lighter. With FXAA on, the picture is enlarged plainly.");
                });
            }, |a, ui| {
                dialog(ui, "Detail", |ui| {
                    bar(ui, "Anisotropic filtering");
                    a.choice(ui, "custom.aniso", &[("", "Game default"), ("4", "4×"), ("8", "8×"), ("16", "16×")]);
                    lead(ui, "Sharper floors and walls seen at an angle, at little cost.");
                    bar(ui, "Texture quality");
                    a.choice(ui, "custom.textures", &[("", "Full"), ("half", "Half"), ("quarter", "Quarter")]);
                    lead(ui, "Half or quarter loads smaller textures: less memory and faster loading on a weak PC, at the cost of detail.");
                });
                dialog(ui, "Effects", |ui| {
                    lead(ui, "The game's own effects, turned off one by one in every quality level once its title shows. Each one off gives the graphics card less to draw; Game default keeps them as the quality level sets them.");
                    for (k, label) in [("shadows", "Shadows"), ("reflections", "Reflections"), ("bloom", "Bloom"), ("vignette", "Vignette")] {
                        bar(ui, label);
                        a.choice(ui, &format!("custom.{k}"), &[("", "Game default"), ("off", "Off")]);
                    }
                });
            }),
            "performance" => self.cols(ui, |a, ui| {
                dialog(ui, "Frame rate", |ui| {
                    bar(ui, "Frame rate");
                    lead(ui, "The game's limit. Higher needs more of the computer.");
                    a.choice(ui, "performance.fps", &[("30", "30"), ("60", "60"), ("120", "120"), ("0", "Unlimited")]);
                    bar(ui, "VSync");
                    a.choice(ui, "custom.vsync", &[("", "Game default"), ("on", "On"), ("off", "Off")]);
                    lead(ui, "On: no tearing, the frame rate follows the screen's. Off: no wait for the screen.");
                    bar(ui, "Idle frame limit");
                    a.choice(ui, "custom.idleFps", &[("", "Off"), ("30", "30"), ("20", "20"), ("10", "10")]);
                    lead(ui, "After 10 seconds without a key, click or stick, the game draws at most this many frames a second: cooler and quieter while it waits. Any input brings the full rate back.");
                });
            }, |a, ui| {
                dialog(ui, "Game code", |ui| {
                    lead(ui, "Compiled translates the game's code once, in the background, and gets faster as you play.");
                    a.choice(ui, "performance.engine", &[("aot", "Compiled"), ("jit", "JIT only")]);
                });
                dialog(ui, "Performance overlay", |ui| {
                    a.choice(ui, "graphics.perf", &[("off", "Off"), ("log", "Log only"), ("overlay", "On screen")]);
                    lead(ui, "Frame rate, frame times, CPU and GPU, in the game's log every second (and on screen). In the game, F9 shows or hides it and F8 records a 30-second summary.");
                });
            }),
            "language" => self.one(ui, 640.0, |a, ui| {
                dialog(ui, "Language", |ui| {
                    bar(ui, "Launcher");
                    let was = a.get("language.ui");
                    a.choice(ui, "language.ui", &[("", "Auto"), ("en", "English"), ("fr", "Français")]);
                    if a.get("language.ui") != was {
                        lang::set(&a.get("language.ui"));
                        send("info", "");
                        a.saves_call(json!({"op": "list"}), Tag::List);
                    }
                    lead(ui, "The language of this launcher's menus. Auto follows your system.");
                    bar(ui, "Text");
                    a.choice(ui, "language.text", &[("", "In-game"), ("10", "English"), ("23", "한국어")]);
                    bar(ui, "Voices");
                    a.choice(ui, "language.voice", &[("", "In-game"), ("10", "English"), ("22", "日本語")]);
                    lead(ui, "Applied when the game starts. In-game keeps the game's own choice. This version shows its text in English or Korean; Japanese is for the voices.");
                });
            }),
            "mods" => {
                self.sublead(ui, "Changes to the game rules, made by the local server (Off is the original game). They apply each time the game starts and never undo what was already earned.", false);
                let onoff = [("false", "Off"), ("true", "On")];
                let mult = [("1", "×1"), ("2", "×2"), ("3", "×3"), ("5", "×5"), ("10", "×10")];
                self.cols(ui, |a, ui| {
                    dialog(ui, "Shop and summons", |ui| {
                        bar(ui, "Unlimited shop stock");
                        a.choice(ui, "mods.shopUnlimited", &onoff);
                        lead(ui, "Shop items that restock daily, weekly or monthly never run out, gems and summon tickets included. Gem packs cost nothing on this server.");
                        bar(ui, "Summon tickets become gems");
                        a.choice(ui, "mods.ticketsToGems", &onoff);
                        bar(ui, "No duplicate ★4");
                        a.choice(ui, "mods.noDuplicateFourStar", &onoff);
                        lead(ui, "A ★4 costume or weapon you already own is swapped for one you don't, from the same summon.");
                        bar(ui, "Pity rewards");
                        a.choice(ui, "mods.pityRewards", &onoff);
                        lead(ui, "Every 100 summon medals become 1,000 gems and 100 Mama Points, sent to the gift box.");
                        bar(ui, "Lost Items give gems");
                        a.choice(ui, "mods.lostItemGems", &onoff);
                        bar(ui, "Hide limited-time sale notices");
                        a.choice(ui, "mods.hideSaleNotices", &onoff);
                        lead(ui, "Stops the \"on sale for a limited time · Ends In\" messages that repeat on the home screen. Applies the next time the game starts.");
                    });
                }, |a, ui| {
                    dialog(ui, "Play", |ui| {
                        bar(ui, "No stamina cost");
                        a.choice(ui, "mods.staminaFree", &onoff);
                        bar(ui, "Free skips");
                        a.choice(ui, "mods.freeSkips", &onoff);
                        bar(ui, "Unlimited daily quests");
                        a.choice(ui, "mods.unlimitedDailyPlays", &onoff);
                        bar(ui, "Rewards");
                        a.choice(ui, "mods.rewardMultiplier", &mult);
                        bar(ui, "EXP");
                        a.choice(ui, "mods.expMultiplier", &mult);
                    });
                });
            }
            "saves" => self.saves_page(ui),
            _ => self.cols(ui, |a, ui| {
                dialog(ui, "Logs", |ui| {
                    match buttons(ui, &[("Logs", true, true), ("Export logs", true, true)]) {
                        Some(0) => send("logs", ""),
                        Some(1) => send("export-logs", ""),
                        _ => {}
                    }
                    let (t, e) = a.export_note.clone();
                    lead_colored(ui, &t, if e { ALERT } else { INK });
                    bar(ui, "Graphics diagnostics");
                    a.choice(ui, "graphics.diagnostics", &[("off", "Off"), ("on", "On")]);
                    lead(ui, "Writes graphics details to the game's log: turn it on when the 3D does not show, play until it happens, then send the zip from Export logs.");
                });
            }, |a, ui| {
                dialog(ui, "Folders", |ui| {
                    a.path_field(ui, "Runtime", "paths.runtime", "bundled: runtime/bin/winapk-runtime.exe");
                    a.path_field(ui, "Runtime data", "paths.runtimeData", "bundled: runtime-data");
                });
                dialog(ui, "Compatibility layer", |ui| {
                    bar(ui, "Run the game with");
                    a.choice(ui, "linux.runner", &[("auto", "Auto"), ("proton", "Proton"), ("umu", "umu-run"), ("wine", "Wine")]);
                    a.path_field(ui, "Proton", "linux.proton", "newest found (GE-Proton first)");
                    if buttons(ui, &[("Prefix", true, true)]) == Some(0) {
                        send("prefix-folder", "");
                    }
                    lead(ui, &a.runner_note.clone());
                    bar(ui, "Environment variables");
                    let mut env = a.get("linux.env");
                    if text_area(ui, &mut env, "MANGOHUD=1").changed() {
                        a.set("linux.env", json!(env));
                    }
                    lead(ui, "One NAME=VALUE per line, given to the game when it starts (Wine/Proton and the runtime). For example MANGOHUD=1 or DXVK_HUD=fps.");
                });
            }),
        }
    }

    fn saves_page(&mut self, ui: &mut Ui) {
        self.sublead(ui, "Your progress is kept in save files. Start over in a new one, switch between them, and change what a save owns. Close the game first: saves change only while it is closed, and a copy of a save is kept before every change.", false);
        let (m, e) = self.saves_msg.clone();
        self.sublead(ui, &m, e);
        self.cols(ui, |a, ui| {
            dialog(ui, "Save files", |ui| {
                if a.slots.is_empty() {
                    lead(ui, if a.saves_busy { "Reading the saves..." } else { "No saves yet." });
                }
                for slot in a.slots.clone() {
                    a.slot_ui(ui, &slot);
                }
                match buttons(ui, &[("Start over", false, true), ("New empty save", true, true)]) {
                    Some(0) => a.keep_pending(Then::Restart),
                    Some(1) => {
                        let n = a.slots.len() + 1;
                        a.ask("New empty save", "A new save that starts from the beginning when you use it. The save in use stays in use. Name:",
                              Some(&trf("Save {}", &[&n])), "Make it", Ask::NewEmpty);
                    }
                    _ => {}
                }
                lead(ui, "Start over makes a new save and plays it from the beginning; the save you have now is kept and can be used again at any time.");
                if buttons(ui, &[("Import from phone", true, true)]) == Some(0) {
                    send("pick", "phone-save");
                }
                lead(ui, "Saves move between the phone (Bunker) and the PC: on the phone, Export save backup, then Import from phone here (it becomes a new save). Export on a save here makes a backup the phone imports.");
            });
        }, |a, ui| {
            dialog(ui, "Resources", |ui| {
                let who = match &a.ed {
                    None => "Choose Edit on a save.".to_string(),
                    Some(ed) => {
                        let pn = ed.player["name"].as_str().unwrap_or("");
                        trf("Editing \"{}\"{}.", &[&a.slot_name(&ed.id), &if pn.is_empty() { String::new() } else { trf(" ({}, Lv {})", &[&pn, &ed.player["level"]]) }])
                    }
                };
                lead(ui, &who);
                for (k, label) in [("freeGems", "Gems"), ("paidGems", "Paid gems"), ("gold", "Gold")] {
                    a.amount_ui(ui, k, label);
                }
                match buttons(ui, &[("Back up now", true, true), ("Backups folder", true, true)]) {
                    Some(0) => {
                        let id = a.ed.as_ref().map(|e| e.id.clone()).unwrap_or_else(|| a.active.clone());
                        let name = a.slot_name(&id);
                        a.saves_do(json!({"op": "backup", "id": id}), Done::Backup(name), After::None);
                    }
                    Some(1) => send("saves-folder", ""),
                    _ => {}
                }
            });
        });
        self.one(ui, 1280.0, |a, ui| a.editor_ui(ui));
    }

    fn slot_ui(&mut self, ui: &mut Ui, slot: &Value) {
        let id = slot["id"].as_str().unwrap_or("").to_string();
        let active = slot["active"].as_bool() == Some(true);
        let started = slot["started"].as_bool() == Some(true);
        let editing = self.ed.as_ref().is_some_and(|e| e.id == id);
        let fill = if active { rgba(233, 228, 216, 0.09) } else { rgba(233, 228, 216, 0.03) };
        let resp = egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, if active { INK } else { EDGE }))
            .inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(slot["name"].as_str().unwrap_or("")).font(font(19.0)).color(INK));
                    if active {
                        ui.add_space(4.0);
                        egui::Frame::new().fill(BAR).inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
                            ui.label(egui::RichText::new(tr("In use")).font(font(13.0)).color(ON_BAR));
                        });
                    }
                });
                let meta = if started {
                    let player = slot["player"].as_str().unwrap_or("");
                    let when = slot["modified"].as_i64().map(when).unwrap_or_default();
                    trf("{} · last played {}", &[&if player.is_empty() { tr("No player yet").to_string() } else { trf("{} · Lv {}", &[&player, &slot["level"]]) }, &when])
                } else {
                    tr("Not played yet: the game starts from the beginning with it").to_string()
                };
                ui.label(egui::RichText::new(meta).font(font(14.0)).color(DIM));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if tool(ui, "Use", !active, false).clicked() {
                        self.keep_pending(Then::Use(id.clone()));
                    }
                    if tool(ui, "Edit", started, false).clicked() {
                        self.keep_pending(Then::Edit(id.clone()));
                    }
                    if tool(ui, "Rename", true, false).clicked() {
                        let name = self.slot_name(&id);
                        self.ask("Rename", "The save's new name:", Some(&name), "Rename", Ask::Rename(id.clone()));
                    }
                    if tool(ui, "Export", started, false).on_hover_text(tr("A backup the phone (Bunker) imports")).clicked() {
                        self.export_id = Some(id.clone());
                        send("pick-save-export", &self.slot_name(&id));
                    }
                    if tool(ui, "Delete", !active, true).clicked() {
                        let name = self.slot_name(&id);
                        self.ask("Delete this save", &trf("Delete \"{}\"? Its progress is no longer offered here. A copy is kept in the backups folder (save/slot-backups) in case you change your mind.", &[&name]),
                                 None, "Delete", Ask::Delete(id.clone()));
                    }
                });
            });
        if editing {
            let r = resp.response.rect.expand(3.0);
            let p = ui.painter();
            let mut x = r.left();
            while x < r.right() {
                p.line_segment([pos2(x, r.top()), pos2((x + 4.0).min(r.right()), r.top())], Stroke::new(1.0, LINE));
                p.line_segment([pos2(x, r.bottom()), pos2((x + 4.0).min(r.right()), r.bottom())], Stroke::new(1.0, LINE));
                x += 8.0;
            }
        }
        ui.add_space(4.0);
    }

    fn amount_ui(&mut self, ui: &mut Ui, key: &str, label: &str) {
        let cur = self.ed.as_ref().and_then(|e| e.player[key].as_i64());
        let changed = self.pending.amounts.contains_key(key);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let (r, _) = ui.allocate_exact_size(vec2(150.0, 30.0), Sense::hover());
            ui.painter().text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, tr(label), font(17.0), if changed { Color32::WHITE } else { SOFT });
            let buf = self.amount_edits.entry(key.to_string()).or_insert_with(|| {
                cur.map(|c| c.to_string()).unwrap_or_default()
            });
            if self.ed.is_none() {
                buf.clear();
            }
            let resp = ui.add_enabled_ui(self.ed.is_some(), |ui| s::number(ui, buf, &cur.map(|c| c.to_string()).unwrap_or_default(), 150.0)).inner;
            if resp.lost_focus() {
                let v = buf.trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.floor() as i64);
                match v {
                    Some(v) if Some(v) != cur => {
                        self.pending.amounts.insert(key.to_string(), v.clamp(0, 2147483647));
                    }
                    _ => {
                        self.pending.amounts.remove(key);
                    }
                }
            }
        });
    }

    fn editor_ui(&mut self, ui: &mut Ui) {
        dialog(ui, "What the save owns", |ui| {
            let on = self.ed.is_some();
            let kinds: Vec<(&str, &str)> = ED_KINDS.to_vec();
            if let Some(k) = choice_grid(ui, &kinds, self.ed_kind) {
                self.ed_kind = ED_KINDS.iter().find(|x| x.0 == k).unwrap().0;
                self.ed_search.clear();
            }
            ui.add_enabled_ui(on, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    text_line(ui, &mut self.ed_search, "Search by name (or character)", 300.0, false);
                    if ["costumes", "weapons"].contains(&self.ed_kind) {
                        select(ui, "ed-rarity", &mut self.ed_rarity, &[("".into(), "Every rarity".into()), ("40".into(), "★4".into()), ("30".into(), "★3".into()), ("20".into(), "★2".into())]);
                    }
                    select(ui, "ed-owned", &mut self.ed_owned, &[("".into(), "Owned and not".into()), ("yes".into(), "Owned".into()), ("no".into(), "Not owned".into())]);
                });
                let levels = LEVEL_KINDS.contains(&self.ed_kind);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let kind = self.ed_kind;
                    if tool(ui, if levels { "Own all shown" } else { "Set all shown to" }, on, false).clicked() {
                        if levels {
                            for x in self.ed_items() {
                                if x.have.is_none() {
                                    self.pending.map(kind).entry(x.id).or_insert(0);
                                }
                            }
                        } else {
                            self.ask("Set all shown", "The amount every item shown will have:", Some("99"), "Set", Ask::SetAllShown);
                        }
                    }
                    if levels && tool(ui, "Max level for all shown", on, false).clicked() {
                        for x in self.ed_items() {
                            self.pending.map(kind).insert(x.id, -1);
                            self.edits.remove(&(kind.to_string(), x.id));
                        }
                    }
                    if kind == "costumes" && tool(ui, "Unlock every character", on, false).clicked() {
                        if let Some(ed) = &self.ed {
                            let have = ed.player["characters"].clone();
                            let add: Vec<i64> = ed.catalog["characters"].as_array().into_iter().flatten()
                                .filter(|c| have.get(c["id"].as_i64().unwrap_or(0).to_string()).is_none())
                                .filter_map(|c| c["costume"].as_i64()).collect();
                            for c in add {
                                self.pending.map("costumes").entry(c).or_insert(0);
                            }
                        }
                    }
                    if levels {
                        egui::Frame::new().stroke(Stroke::new(1.0, Color32::from_rgb(0x7d, 0x79, 0x72))).inner_margin(egui::Margin::symmetric(12, 2)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tr("Level for all shown")).font(font(14.0)));
                                let r = s::number(ui, &mut self.ed_bulk_level, tr("Lv"), 50.0);
                                if r.lost_focus() {
                                    if let Ok(v) = self.ed_bulk_level.trim().parse::<i64>() {
                                        if v >= 1 {
                                            let cap = if kind == "companions" { 50 } else { 100 };
                                            for x in self.ed_items() {
                                                self.pending.map(kind).insert(x.id, v.min(cap));
                                                self.edits.remove(&(kind.to_string(), x.id));
                                            }
                                        }
                                    }
                                    self.ed_bulk_level.clear();
                                }
                            });
                        });
                    }
                });
            });
            let items = self.ed_items();
            self.tiles(ui, &items);
            if on {
                lead(ui, &trf("{} shown.", &[&items.len()]));
            }
            bar(ui, "Changes");
            let pending = self.pending_text();
            lead(ui, if pending.is_empty() { "None yet." } else { &pending });
            let has = !pending.is_empty();
            match buttons(ui, &[("Apply to the save", false, has), ("Discard", true, has)]) {
                Some(0) => {
                    let name = self.ed.as_ref().map(|e| self.slot_name(&e.id)).unwrap_or_default();
                    self.ask("Apply the changes", &trf("Change \"{}\": {}. A copy of the save is kept first (save/slot-backups).", &[&name, &pending]), None, "Apply", Ask::Apply);
                }
                Some(1) => {
                    self.pending = Pending::default();
                    self.edits.clear();
                    self.amount_edits.clear();
                    self.saves_msg("", false);
                }
                _ => {}
            }
            let (m, e) = self.saves_msg.clone();
            lead_colored(ui, &m, if e { ALERT } else { INK });
            lead(ui, "Owning a costume unlocks its character. Weapons are listed in their first form: evolve them in the game. Levels above what a costume or weapon can reach now ascend it as far as needed (no materials are used). Owned things are not taken away.");
        });
    }

    fn pic(&mut self, key: &str) -> Option<TextureHandle> {
        if key.is_empty() || !self.pic_have.contains(key) {
            return None;
        }
        self.pics.get(&format!("launcher/ui/game/icons/{key}.png"))
    }

    fn draw_pic(&mut self, ui: &Ui, rect: Rect, key: &str, alt: &str, contain: bool, gray: bool) {
        let p = ui.painter();
        p.rect_filled(rect, 0.0, Color32::from_rgb(0x1f, 0x1c, 0x1a));
        match self.pic(key) {
            Some(t) => {
                let (tw, th) = (t.size()[0] as f32, t.size()[1] as f32);
                let (r, uv) = if contain {
                    let sc = (rect.width() / tw).min(rect.height() / th);
                    (Rect::from_center_size(rect.center(), vec2(tw * sc, th * sc)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))
                } else {
                    let (fa, ta) = (rect.width() / rect.height(), tw / th);
                    let uv = if ta > fa { let w = fa / ta; Rect::from_min_max(pos2(0.5 - w / 2.0, 0.0), pos2(0.5 + w / 2.0, 1.0)) }
                             else { let h = ta / fa; Rect::from_min_max(pos2(0.0, 0.5 - h / 2.0), pos2(1.0, 0.5 + h / 2.0)) };
                    (rect, uv)
                };
                let tint = if gray { Color32::from_gray(110) } else { Color32::WHITE };
                p.image(t.id(), r, uv, tint);
            }
            None => {
                let mut x = rect.left() - rect.height();
                let clip = p.with_clip_rect(rect);
                while x < rect.right() {
                    clip.line_segment([pos2(x, rect.bottom()), pos2(x + rect.height(), rect.top())], Stroke::new(6.0, Color32::from_rgb(0x34, 0x31, 0x2e)));
                    x += 17.0;
                }
                p.text(rect.center(), Align2::CENTER_CENTER, alt.chars().next().map(String::from).unwrap_or_default(), font(30.0), FRAME);
            }
        }
    }

    fn tiles(&mut self, ui: &mut Ui, items: &[Item]) {
        let kind = self.ed_kind;
        let levels = LEVEL_KINDS.contains(&kind);
        let w = ui.available_width();
        let line = |ui: &Ui, y: f32| ui.painter().line_segment([pos2(ui.min_rect().left(), y), pos2(ui.min_rect().left() + w, y)], Stroke::new(1.0, EDGE));
        line(ui, ui.cursor().top());
        if self.ed.is_none() {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| ui.label(egui::RichText::new(tr("Choose Edit on a save (one played at least once).")).font(font(14.0)).color(DIM)));
            ui.add_space(10.0);
            line(ui, ui.cursor().top());
            return;
        }
        let shown = &items[..items.len().min(ROW_LIMIT)];
        let cols = ((w - 8.0 + 10.0) / 138.0).floor().max(1.0) as usize;
        let tw = (w - 8.0 - 10.0 * (cols as f32 - 1.0)) / cols as f32;
        let th = tw - 12.0 + 6.0 * 2.0 + 20.0 * 2.0 + 16.0 + 16.0 + 30.0;
        let rows = shown.len().div_ceil(cols);
        egui::ScrollArea::vertical().id_salt("ed-list").max_height(620.0).min_scrolled_height(620.0_f32.min(rows as f32 * (th + 10.0) + 40.0)).auto_shrink([false, true]).show_rows(ui, th + 10.0, rows, |ui, range| {
            for row in range {
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.x = 10.0;
                    for x in shown.iter().skip(row * cols).take(cols) {
                        self.tile(ui, x, kind, levels, tw, th);
                    }
                });
                ui.add_space(10.0);
            }
            if items.len() > ROW_LIMIT {
                ui.vertical_centered(|ui| ui.label(egui::RichText::new(trf("Showing {} of {}: search to find the rest.", &[&ROW_LIMIT, &items.len()])).font(font(14.0)).color(DIM)));
            }
            if items.is_empty() {
                ui.vertical_centered(|ui| ui.label(egui::RichText::new(tr("Nothing matches.")).font(font(14.0)).color(DIM)));
            }
        });
        line(ui, ui.cursor().top());
    }

    fn tile(&mut self, ui: &mut Ui, x: &Item, kind: &'static str, levels: bool, tw: f32, th: f32) {
        let lv = self.pending.get(kind).and_then(|m| m.get(&x.id)).copied();
        let has = lv.is_some();
        let owned = x.have.is_some();
        let (rect, _) = ui.allocate_exact_size(vec2(tw, th), Sense::hover());
        let p = ui.painter().clone();
        let border = if has { Color32::WHITE } else if owned { Color32::from_rgb(0x8d, 0x89, 0x81) } else { Color32::from_rgb(0x4d, 0x49, 0x45) };
        p.rect_filled(rect, 0.0, if has { rgba(233, 228, 216, 0.1) } else { rgba(233, 228, 216, 0.03) });
        p.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
        let inner = rect.shrink(6.0);
        let frame = Rect::from_min_size(inner.min, vec2(inner.width(), inner.width()));
        self.draw_pic(ui, frame, &x.img, &x.name, true, !owned && !has && levels);
        let mut sub = x.sub.clone();
        if let Some(r) = rarity(x.rarity) {
            sub = if sub.is_empty() { r.into() } else { format!("{sub} · {r}") };
        }
        let resp = ui.interact(frame, ui.scope_id().with(("tile", x.id)), if levels && !owned { Sense::click() } else { Sense::hover() })
            .on_hover_text(if sub.is_empty() { x.name.clone() } else { format!("{}\n{sub}", x.name) });
        if levels {
            let c = pos2(frame.right() - 6.0 - 7.0, frame.top() + 6.0 + 7.0);
            let d = |h: f32| vec![pos2(c.x, c.y - h), pos2(c.x + h, c.y), pos2(c.x, c.y + h), pos2(c.x - h, c.y)];
            let (fill, stroke) = if owned { (INK, INK) } else if has { (CREAM, Color32::WHITE) } else { (rgba(30, 27, 25, 0.7), Color32::from_rgb(0x7d, 0x79, 0x72)) };
            p.add(egui::Shape::convex_polygon(d(9.0), fill, Stroke::new(2.0, stroke)));
            if resp.clicked() {
                let m = self.pending.map(kind);
                if m.remove(&x.id).is_none() {
                    m.insert(x.id, 0);
                }
            }
        }
        let mut y = frame.bottom() + 2.0;
        let name = s::wrapped(ui, &x.name, 14.0, INK, inner.width());
        let name_h = two_lines(&name);
        p.with_clip_rect(Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), name_h))).galley(pos2(inner.left(), y), name, INK);
        y += name_h + 2.0;
        let small = |p: &egui::Painter, y: f32, t: &str, c: Color32| {
            p.with_clip_rect(Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 16.0)))
                .text(pos2(inner.left(), y), Align2::LEFT_TOP, t, font(12.0), c);
        };
        if levels {
            small(&p, y, &sub, DIM);
            y += 16.0;
            let now = match &x.have {
                Some(h) => trf("Owned · Lv {}{}", &[&h["level"], &h["lb"].as_i64().filter(|n| *n > 0).map(|n| format!(" · {n}✦")).unwrap_or_default()]),
                None if has => tr("To own").into(),
                None => tr("Not owned").into(),
            };
            small(&p, y, &now, Color32::from_rgb(0xb9, 0xb5, 0xab));
        } else {
            let now = trf("Now {}", &[&x.have.as_ref().and_then(|h| h["count"].as_i64()).map(thousands).unwrap_or_else(|| "0".into())]);
            small(&p, y, &now, Color32::from_rgb(0xb9, 0xb5, 0xab));
        }
        let row = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 26.0), inner.max);
        let key = (kind.to_string(), x.id);
        let shown = match lv {
            Some(v) if levels && v > 0 => v.to_string(),
            Some(v) if !levels => v.to_string(),
            _ => String::new(),
        };
        let buf = self.edits.entry(key.clone()).or_insert(shown).clone();
        let mut text = buf;
        let mut commit = None;
        let mut max_clicked = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let hint = if levels { tr("Lv").to_string() } else { x.have.as_ref().and_then(|h| h["count"].as_i64()).unwrap_or(0).to_string() };
                let w = if levels { row.width() - 48.0 } else { row.width() };
                let r = s::number(ui, &mut text, &hint, w - 12.0);
                if r.lost_focus() {
                    commit = Some(text.clone());
                }
                if levels {
                    let on = lv == Some(-1);
                    let t = tool(ui, "Max", true, false);
                    if on {
                        ui.painter().rect_filled(t.rect, 0.0, CREAM);
                        ui.painter().text(t.rect.center(), Align2::CENTER_CENTER, "Max", font(14.0), ON_BAR);
                    }
                    max_clicked = t.clicked();
                }
            });
        });
        self.edits.insert(key.clone(), text);
        let m = self.pending.map(kind);
        if let Some(t) = commit {
            let v = t.trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.floor() as i64);
            if levels {
                match v {
                    Some(v) if v >= 1 => {
                        m.insert(x.id, v.min(if kind == "companions" { 50 } else { 100 }));
                    }
                    _ if owned => {
                        m.remove(&x.id);
                    }
                    _ => {
                        m.insert(x.id, 0);
                    }
                }
            } else {
                let now = x.have.as_ref().and_then(|h| h["count"].as_i64()).unwrap_or(0);
                match v {
                    Some(v) if v != now => {
                        m.insert(x.id, v.clamp(0, 2147483647));
                    }
                    _ => {
                        m.remove(&x.id);
                    }
                }
            }
        }
        if max_clicked {
            if m.get(&x.id) == Some(&-1) {
                if owned { m.remove(&x.id); } else { m.insert(x.id, 0); }
            } else {
                m.insert(x.id, -1);
            }
            self.edits.remove(&key);
        }
    }

    fn events_page(&mut self, ui: &mut Ui) {
        self.sublead(ui, "Choose the summons and events the game offers. Everything is closed until you open it, except the game's permanent summons (the chapter summons and the ticket summons), which are always on. Summons change the next time you open Summons in the game; events change the next time the game starts. The pictures are made from your own game files the first time this page opens.", false);
        let (m, e) = self.ct_msg.clone();
        self.sublead(ui, &m, e);
        self.one(ui, 1280.0, |a, ui| {
            dialog(ui, "Summons", |ui| a.ct_list(ui, 0));
            dialog(ui, "Events", |ui| {
                a.ct_list(ui, 1);
                lead(ui, "Permanent are the daily and lasting quests (Karma Stone, Guerrilla, Dungeons, Character Quests, Abyss Tower, Fate Boards); Record, Variation and Bonus are the game's past events. Their shops stay open either way.");
            });
        });
    }

    fn ct_name(kind: usize, x: &Value) -> String {
        let name = x["name"].as_str().unwrap_or("");
        if !name.is_empty() {
            return name.to_string();
        }
        if kind == 0 {
            let f: Vec<&str> = x["featured"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if !f.is_empty() { f.join(", ") } else { trf("Summon {}", &[&x["id"]]) }
        } else {
            trf("Event {}", &[&x["id"]])
        }
    }
    fn ct_sub(kind: usize, x: &Value) -> String {
        if kind == 0 {
            let f: Vec<&str> = x["featured"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            [x["kind"].as_str().unwrap_or("").to_string(), if f.is_empty() { String::new() } else { trf("Featured: {}", &[&f.join(", ")]) },
             x["note"].as_str().unwrap_or("").to_string()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
        } else {
            x["group"].as_str().unwrap_or("").to_string()
        }
    }
    fn ct_shown(&self, kind: usize) -> Vec<usize> {
        let list = if kind == 0 { &self.ct_banners } else { &self.ct_events };
        let f = &self.ct_filter[kind];
        let q = f.search.trim().to_lowercase();
        list.iter().enumerate().filter(|(_, x)| {
            let group = if kind == 0 { x["kind"].as_str() } else { x["group"].as_str() }.unwrap_or("");
            let find = if kind == 0 {
                let mut v = vec![Self::ct_name(0, x)];
                v.extend(x["featured"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from));
                v.push(x["key"].as_str().unwrap_or("").into());
                v.push(x["id"].to_string());
                v.join(" | ").to_lowercase()
            } else {
                format!("{} | {}", Self::ct_name(1, x), x["id"]).to_lowercase()
            };
            (f.group.is_empty() || group == f.group) && (f.state.is_empty() || (f.state == "on") == (x["on"].as_bool() == Some(true)))
                && (q.is_empty() || find.contains(&q))
        }).map(|(i, _)| i).collect()
    }

    fn ct_list(&mut self, ui: &mut Ui, kind: usize) {
        let groups: Vec<String> = {
            let list = if kind == 0 { &self.ct_banners } else { &self.ct_events };
            let mut g: Vec<String> = Vec::new();
            for x in list {
                let v = if kind == 0 { x["kind"].as_str() } else { x["group"].as_str() }.unwrap_or("").to_string();
                if !g.contains(&v) {
                    g.push(v);
                }
            }
            g
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let f = &mut self.ct_filter[kind];
            text_line(ui, &mut f.search, if kind == 0 { "Search summons by name or featured character" } else { "Search events by name" }, 340.0, false);
            let mut opts = vec![(String::new(), "Every kind".to_string())];
            opts.extend(groups.iter().map(|g| (g.clone(), g.clone())));
            select(ui, &format!("ct-group{kind}"), &mut f.group, &opts);
            select(ui, &format!("ct-state{kind}"), &mut f.state, &[("".into(), "Open and closed".into()), ("on".into(), "Open".into()), ("off".into(), "Closed".into())]);
        });
        let shown = self.ct_shown(kind);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (label, on) in [("Open all shown", true), ("Close all shown", false)] {
                if tool(ui, label, true, false).clicked() && !shown.is_empty() {
                    let list = if kind == 0 { &mut self.ct_banners } else { &mut self.ct_events };
                    for &i in &shown {
                        if list[i]["always"].as_bool() != Some(true) {
                            list[i]["on"] = json!(on);
                        }
                    }
                    self.ct_save();
                }
            }
            let list = if kind == 0 { &self.ct_banners } else { &self.ct_events };
            let open = list.iter().filter(|x| x["on"].as_bool() == Some(true)).count();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(trf("{} shown · {} of {} open", &[&shown.len(), &open, &list.len()])).font(font(14.0)).color(DIM));
            });
        });
        let w = ui.available_width();
        let (min, aspect) = if kind == 0 { (236.0, 118.0 / 234.0) } else { (168.0, 490.0 / 402.0) };
        let cols = ((w - 8.0 + 12.0) / (min + 12.0)).floor().max(1.0) as usize;
        let pw = (w - 8.0 - 12.0 * (cols as f32 - 1.0)) / cols as f32;
        let ph = 12.0 + (pw - 12.0) * aspect + 6.0 + 40.0 + 32.0;
        let rows = shown.len().div_ceil(cols);
        let top = ui.cursor().top();
        ui.painter().line_segment([pos2(ui.min_rect().left(), top), pos2(ui.min_rect().left() + w, top)], Stroke::new(1.0, EDGE));
        egui::ScrollArea::vertical().id_salt(("ct", kind)).max_height(620.0).min_scrolled_height(620.0_f32.min(rows.max(1) as f32 * (ph + 24.0))).auto_shrink([false, true]).show_rows(ui, ph + 12.0, rows.max(1), |ui, range| {
            if shown.is_empty() {
                ui.add_space(10.0);
                ui.vertical_centered(|ui| ui.label(egui::RichText::new(tr("Nothing matches.")).font(font(14.0)).color(DIM)));
                return;
            }
            for row in range {
                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for &i in shown.iter().skip(row * cols).take(cols) {
                        if self.ct_pic(ui, kind, i, pw, ph, aspect) {
                            let list = if kind == 0 { &mut self.ct_banners } else { &mut self.ct_events };
                            if list[i]["always"].as_bool() != Some(true) {
                                let on = list[i]["on"].as_bool() == Some(true);
                                list[i]["on"] = json!(!on);
                                self.ct_save();
                            }
                        }
                    }
                });
                ui.add_space(6.0);
            }
        });
        let bottom = ui.cursor().top();
        ui.painter().line_segment([pos2(ui.min_rect().left(), bottom), pos2(ui.min_rect().left() + w, bottom)], Stroke::new(1.0, EDGE));
    }

    fn ct_pic(&mut self, ui: &mut Ui, kind: usize, i: usize, pw: f32, ph: f32, aspect: f32) -> bool {
        let x = if kind == 0 { self.ct_banners[i].clone() } else { self.ct_events[i].clone() };
        let (on, always) = (x["on"].as_bool() == Some(true), x["always"].as_bool() == Some(true));
        let name = Self::ct_name(kind, &x);
        let sub = Self::ct_sub(kind, &x);
        let (rect, resp) = ui.allocate_exact_size(vec2(pw, ph), if always { Sense::hover() } else { Sense::click() });
        let p = ui.painter().clone();
        let fill = if on || (always && resp.hovered()) { rgba(233, 228, 216, 0.1) } else if resp.hovered() { rgba(233, 228, 216, 0.09) } else { rgba(233, 228, 216, 0.03) };
        p.rect_filled(rect, 0.0, fill);
        let border = if on { INK } else { Color32::from_rgb(0x4d, 0x49, 0x45) };
        if always {
            let r = rect.shrink(0.5);
            for (a, b) in [(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] {
                let n = ((b - a).length() / 6.0) as i32;
                for k in (0..n).step_by(2) {
                    let t0 = k as f32 / n as f32;
                    let t1 = ((k + 1) as f32 / n as f32).min(1.0);
                    p.line_segment([a.lerp(b, t0), a.lerp(b, t1)], Stroke::new(1.0, border));
                }
            }
        } else {
            p.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
        }
        let inner = rect.shrink(6.0);
        let frame = Rect::from_min_size(inner.min, vec2(inner.width(), inner.width() * aspect));
        let img = x["img"].as_str().unwrap_or("").to_string();
        self.draw_pic(ui, frame, &img, &name, false, !on);
        let state = tr(if always { "Always on" } else if on { "Open" } else { "Closed" });
        let g = s::spaced(ui, state, 12.0, Color32::BLACK, 1.0);
        let chip = Rect::from_min_size(pos2(frame.left(), frame.bottom() - 18.0), vec2(g.size().x + 20.0, 18.0));
        let (cf, cc) = if always { (BAR, ON_BAR) } else if on { (CREAM, ON_BAR) } else { (rgba(30, 27, 25, 0.85), DIM) };
        p.rect_filled(chip, 0.0, cf);
        p.galley(chip.center() - g.size() / 2.0, s::spaced(ui, state, 12.0, cc, 1.0), cc);
        let mut y = frame.bottom() + 3.0;
        let ng = s::wrapped(ui, &name, 15.0, if x["name"].as_str().unwrap_or("").is_empty() { DIM } else { INK }, inner.width());
        let nh = two_lines(&ng);
        p.with_clip_rect(Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), nh))).galley(pos2(inner.left(), y), ng, INK);
        y += nh + 3.0;
        let sg = s::wrapped(ui, &sub, 12.0, DIM, inner.width());
        p.with_clip_rect(Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), rect.bottom() - 4.0))).galley(pos2(inner.left(), y), sg, DIM);
        let tip = format!("{name}{}\n{sub}", if always { tr(" (always on: one of the game's permanent summons)") } else { "" });
        resp.on_hover_text(tip).clicked()
    }

    fn controls_page(&mut self, ui: &mut Ui) {
        self.cols(ui, |a, ui| {
            dialog(ui, "Bindings", |ui| {
                bar(ui, "Keyboard and controller");
                a.choice(ui, "controls.enabled", &[("true", "On"), ("false", "Off")]);
                a.binds_table(ui);
                match buttons(ui, &[("Add", false, true), ("Defaults", true, true)]) {
                    Some(0) => {
                        let scene = a.scene;
                        if let Some(b) = a.st["controls"]["binds"].as_array_mut() {
                            b.push(json!({"name": tr("New action"), "scene": scene, "vk": 0, "key": "", "pad": "", "x": 0.5, "y": 0.5}));
                            a.selected = Sel::Bind(b.len() - 1);
                        }
                        a.save();
                    }
                    Some(1) => {
                        a.st["controls"]["binds"] = st::default_binds();
                        a.st["controls"]["move"] = st::default_move();
                        st::free_toggle(&mut a.st);
                        a.selected = Sel::Move;
                        a.save();
                    }
                    _ => {}
                }
            });
            dialog(ui, "Controller", |ui| {
                bar(ui, "Mouse mode button");
                lead(ui, "Switches the controller between its touch layout and a mouse: a cursor on the screen, left stick to move it, A to click (hold to drag), right stick to scroll, B for Back. Press it again for the touch layout.");
                let opts: Vec<(&str, &str)> = st::TOGGLES.iter().map(|t| (*t, *t)).collect();
                let cur = a.get("controls.pad.mouseToggle");
                if let Some(v) = choice_grid(ui, &opts, &cur) {
                    a.st["controls"]["pad"]["mouseToggle"] = json!(v);
                    st::free_toggle(&mut a.st);
                    a.save();
                }
                bar(ui, "Cursor speed");
                let mut v = a.st["controls"]["pad"]["mouseSpeed"].as_f64().unwrap_or(1.0);
                let label = format!("×{v:.1}");
                if slider(ui, &mut v, 0.4, 2.5, 0.1, &label) {
                    a.st["controls"]["pad"]["mouseSpeed"] = json!((v * 10.0).round() / 10.0);
                    a.save();
                }
            });
        }, |a, ui| {
            dialog(ui, "Screen", |ui| {
                bar(ui, "Touch points");
                if let Some(v) = choice_grid(ui, &[("field", "Exploring"), ("battle", "Battle"), ("dialog", "Choices")], a.scene) {
                    a.scene = match v.as_str() { "battle" => "battle", "dialog" => "dialog", _ => "field" };
                }
                lead(ui, "Select a row, then click the screen to place its point. Each screen shows its own buttons.");
                a.screen(ui);
                lead(ui, "W A S D or the left stick drag from Move to run; hold Alt to walk. The mouse is a finger. Esc or pad B is Back.");
                bar(ui, "Move reach");
                let mut r = a.st["controls"]["move"]["radius"].as_f64().unwrap_or(0.15);
                let label = format!("{}%", (r * 100.0).round());
                if slider(ui, &mut r, 0.05, 0.3, 0.01, &label) {
                    a.st["controls"]["move"]["radius"] = json!((r * 100.0).round() / 100.0);
                    a.save();
                }
            });
        });
    }

    fn binds_table(&mut self, ui: &mut Ui) {
        let w = ui.available_width();
        let cols = [w * 0.36, w * 0.18, w * 0.24, w * 0.22];
        let header = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (i, h) in ["Action", "Key", "Pad", "Point"].iter().enumerate() {
                    let (r, _) = ui.allocate_exact_size(vec2(cols[i], 26.0), Sense::hover());
                    ui.painter().text(pos2(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, tr(h), font(14.0), DIM);
                }
            });
            let y = ui.cursor().top();
            ui.painter().line_segment([pos2(ui.min_rect().left(), y), pos2(ui.min_rect().left() + w, y)], Stroke::new(1.0, EDGE));
        };
        header(ui);
        let toggle = self.get("controls.pad.mouseToggle");
        let pos = |v: &Value| format!("{}%, {}%", (st::num(v, "x") * 100.0).round(), (st::num(v, "y") * 100.0).round());
        let n = self.st["controls"]["binds"].as_array().map_or(0, Vec::len);
        let mut remove = None;
        for row in 0..=n {
            let sel = if row == 0 { self.selected == Sel::Move } else { self.selected == Sel::Bind(row - 1) };
            let top = ui.cursor().top();
            let row_rect = Rect::from_min_size(pos2(ui.min_rect().left(), top), vec2(w, 36.0));
            let bg = ui.painter().add(egui::Shape::Noop);
            let fg = if sel { ON_BAR } else { INK };
            let mut clicked_row = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.set_height(36.0);
                if row == 0 {
                    for (i, t) in ["Move", "W A S D", "L stick", &pos(&self.st["controls"]["move"])].iter().enumerate() {
                        let (r, _) = ui.allocate_exact_size(vec2(cols[i], 36.0), Sense::hover());
                        ui.painter().text(pos2(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, tr(t), font(if i == 0 { 16.0 } else { 14.0 }), fg);
                    }
                } else {
                    let i = row - 1;
                    let b = self.st["controls"]["binds"][i].clone();
                    let mut name = tr(b["name"].as_str().unwrap_or("")).to_string();
                    ui.add_space(8.0);
                    let r = ui.add(egui::TextEdit::singleline(&mut name).frame(egui::Frame::NONE).desired_width(cols[0] - 16.0).font(font(16.0)).text_color(fg));
                    if r.changed() {
                        self.st["controls"]["binds"][i]["name"] = json!(name);
                        self.save();
                    }
                    if r.gained_focus() {
                        clicked_row = true;
                    }
                    ui.add_space(8.0);
                    let listen = self.listening == Some(i);
                    let label = if listen { tr("Press a key").to_string() } else { b["key"].as_str().filter(|k| !k.is_empty()).unwrap_or("—").to_string() };
                    let (kr, kresp) = ui.allocate_exact_size(vec2(cols[1], 36.0), Sense::click());
                    let g = s::spaced(ui, &label, 13.0, fg, 0.0);
                    let bw = g.size().x.max(48.0) + 16.0;
                    let br = Rect::from_min_size(pos2(kr.left() + 8.0, kr.center().y - 12.0), vec2(bw.min(kr.width() - 8.0), 24.0));
                    if listen {
                        ui.painter().rect_filled(br, 0.0, ALERT);
                    }
                    ui.painter().rect_stroke(br, 0.0, Stroke::new(1.0, if listen { ALERT } else { fg }), StrokeKind::Inside);
                    ui.painter().text(br.center(), Align2::CENTER_CENTER, &label, font(13.0), if listen { DIALOG } else { fg });
                    if kresp.clicked() {
                        self.listening = Some(i);
                    }
                    let mut pad = b["pad"].as_str().unwrap_or("").to_string();
                    let opts: Vec<(String, String)> = st::PADS.iter().filter(|p| **p != toggle)
                        .map(|p| (p.to_string(), if p.is_empty() { "—".into() } else { p.to_string() })).collect();
                    ui.allocate_ui(vec2(cols[2], 36.0), |ui| {
                        ui.add_space(4.0);
                        if select(ui, &format!("pad{i}"), &mut pad, &opts) {
                            self.st["controls"]["binds"][i]["pad"] = json!(pad);
                            self.save();
                        }
                    });
                    let (pr, _) = ui.allocate_exact_size(vec2(cols[3] - 30.0, 36.0), Sense::hover());
                    ui.painter().text(pos2(pr.left() + 8.0, pr.center().y), Align2::LEFT_CENTER, pos(&b), font(14.0), fg);
                    let (dr, dresp) = ui.allocate_exact_size(vec2(30.0, 36.0), Sense::click());
                    s::cross(ui.painter(), dr.center(), 5.0, fg.gamma_multiply(if dresp.hovered() { 1.0 } else { 0.6 }));
                    if dresp.on_hover_text(tr("Remove")).clicked() {
                        remove = Some(i);
                    }
                }
            });
            let resp = ui.interact(row_rect, ui.scope_id().with(("bindrow", row)), Sense::click());
            if sel {
                ui.painter().set(bg, egui::Shape::rect_filled(row_rect, 0.0, BAR));
            } else if resp.hovered() {
                ui.painter().set(bg, egui::Shape::rect_filled(row_rect, 0.0, rgba(233, 228, 216, 0.06)));
            }
            ui.painter().line_segment([row_rect.left_bottom(), row_rect.right_bottom()], Stroke::new(1.0, Color32::from_rgb(0x3d, 0x3a, 0x37)));
            if resp.clicked() || clicked_row {
                if row == 0 {
                    self.selected = Sel::Move;
                    self.scene = "field";
                } else if self.selected != Sel::Bind(row - 1) {
                    self.selected = Sel::Bind(row - 1);
                    self.scene = match self.st["controls"]["binds"][row - 1]["scene"].as_str() { Some("battle") => "battle", Some("dialog") => "dialog", _ => "field" };
                }
            }
        }
        if let Some(i) = remove {
            if let Some(b) = self.st["controls"]["binds"].as_array_mut() {
                b.remove(i);
            }
            self.selected = Sel::Move;
            self.listening = None;
            self.save();
        }
    }

    fn screen(&mut self, ui: &mut Ui) {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::click());
        let p = ui.painter().clone();
        p.rect_filled(rect, 0.0, Color32::from_rgb(0x3a, 0x35, 0x32));
        let shot = format!("launcher/ui/game/{}.jpg", self.scene);
        let have_shot = crate::path(&shot).is_file();
        if let Some(t) = have_shot.then(|| self.pics.get(&shot)).flatten() {
            p.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        } else {
            let mut y = rect.top();
            while y < rect.bottom() {
                p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0, rgba(255, 255, 255, 0.04)));
                y += 40.0;
            }
            let mut x = rect.left();
            while x < rect.right() {
                p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, rgba(255, 255, 255, 0.04)));
                x += 40.0;
            }
        }
        p.rect_stroke(rect, 0.0, Stroke::new(2.0, FRAME), StrokeKind::Inside);
        let at = |v: &Value| pos2(rect.left() + st::num(v, "x") as f32 * rect.width(), rect.top() + st::num(v, "y") as f32 * rect.height());
        let clip = p.with_clip_rect(rect);
        if self.scene == "field" {
            let c = at(&self.st["controls"]["move"]);
            let sel = self.selected == Sel::Move;
            let h = 35.0;
            let pts = vec![pos2(c.x, c.y - h), pos2(c.x + h, c.y), pos2(c.x, c.y + h), pos2(c.x - h, c.y)];
            clip.add(egui::Shape::convex_polygon(pts.clone(), if sel { rgba(233, 230, 217, 0.85) } else { rgba(43, 40, 38, 0.4) }, Stroke::NONE));
            clip.add(egui::Shape::dashed_line(&[pts[0], pts[1], pts[2], pts[3], pts[0]], Stroke::new(2.0, LINE), 6.0, 4.0));
        }
        let binds = self.st["controls"]["binds"].as_array().cloned().unwrap_or_default();
        for (i, b) in binds.iter().enumerate() {
            if b["scene"].as_str().unwrap_or("field") != self.scene {
                continue;
            }
            let sel = self.selected == Sel::Bind(i);
            let label = b["key"].as_str().filter(|k| !k.is_empty()).or(b["name"].as_str().map(tr)).unwrap_or("").to_string();
            let g = s::spaced(ui, &label, 12.0, if sel { ON_BAR } else { INK }, 0.0);
            let r = Rect::from_center_size(at(b), vec2((g.size().x + 18.0).max(30.0), 30.0));
            clip.rect_filled(r, 0.0, if sel { CREAM } else { DIALOG });
            clip.rect_stroke(r, 0.0, Stroke::new(2.0, LINE), StrokeKind::Inside);
            clip.galley(r.center() - g.size() / 2.0, g, INK);
        }
        if let Some(pos) = resp.clicked().then(|| resp.interact_pointer_pos()).flatten() {
            let x = (((pos.x - rect.left()) / rect.width()) as f64 * 1000.0).round() / 1000.0;
            let y = (((pos.y - rect.top()) / rect.height()) as f64 * 1000.0).round() / 1000.0;
            let target = match self.selected {
                Sel::Move => self.st["controls"].get_mut("move"),
                Sel::Bind(i) => self.st["controls"]["binds"].get_mut(i),
            };
            if let Some(t) = target {
                t["x"] = json!(x);
                t["y"] = json!(y);
                self.save();
            }
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    fn modal_ui(&mut self, ctx: &egui::Context) {
        let Some(m) = &mut self.modal else { return };
        let mut answer: Option<Option<String>> = None;
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("modal-shade")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, rgba(20, 17, 15, 0.7));
        });
        let w = 640.0_f32.min(screen.width() * 0.9);
        egui::Area::new(egui::Id::unique("modal")).order(egui::Order::Tooltip).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_width(w);
            dialog(ui, &m.title.clone(), |ui| {
                lead(ui, &m.text);
                let mut enter = false;
                if let Some(input) = &mut m.input {
                    let r = ui.add(egui::TextEdit::singleline(input).frame(egui::Frame::NONE).desired_width(ui.available_width()).font(font(17.0)).text_color(INK).char_limit(40).margin(vec2(4.0, 6.0)));
                    if m.focus {
                        r.request_focus();
                        m.focus = false;
                    }
                    enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    answer = Some(None);
                }
                match buttons(ui, &[(&m.ok.clone(), false, true), ("Cancel", true, true)]) {
                    Some(0) => enter = true,
                    Some(1) => answer = Some(None),
                    _ => {}
                }
                if enter {
                    match &m.input {
                        Some(t) if t.trim().is_empty() => m.focus = true,
                        Some(t) => answer = Some(Some(t.trim().to_string())),
                        None => answer = Some(Some(String::new())),
                    }
                }
            });
        });
        if let Some(a) = answer {
            let m = self.modal.take().unwrap();
            if let Some(text) = a {
                self.answer(m.ask, m.input.map(|_| text));
            }
        }
    }
}

fn rarity(r: i64) -> Option<&'static str> {
    match r {
        10 => Some("★1"),
        20 => Some("★2"),
        30 => Some("★3"),
        40 => Some("★4"),
        50 => Some("★5"),
        _ => None,
    }
}

fn when(ms: i64) -> String {
    let t = (ms / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{}/{:02}/{:02} {:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}

fn two_lines(g: &egui::Galley) -> f32 {
    g.rows.iter().take(2).map(|r| r.rect().max.y).fold(0.0, f32::max)
}
