use crate::sh;
use crate::state::{self, History, DECK_HISTORY};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const SERVICES: [&str; 2] = ["streamdeck-ctl.service", "streamdeck-ctl-deck.service"];
const GESTURES: [&str; 3] = ["tap", "long", "double"];
const POSITIONS: [&str; 3] = ["left", "center", "right"];
const PREV_INDEX: u8 = 10;
const NEXT_INDEX: u8 = 14;

fn layout(kind: &str) -> (u8, u8) {
    match kind {
        "XL" => (8, 4),
        "Mini" => (3, 2),
        "Plus" => (4, 2),
        _ => (5, 3),
    }
}

#[derive(Serialize)]
pub struct Device {
    pub kind: String,
    pub serial: String,
    pub cols: u8,
    pub rows: u8,
    pub pedal: bool,
}

#[derive(Serialize)]
pub struct Key {
    pub index: u8,
    /// "button" for a configured key, "page" for a pagination arrow, "blank" otherwise
    pub kind: String,
    pub preview: String,
    pub label: String,
    pub glyph: String,
    pub icon: String,
    pub bg: String,
    pub fg: String,
    pub action: String,
    pub target: String,
}

#[derive(Serialize)]
pub struct Page {
    pub name: String,
    pub keys: Vec<Key>,
}

#[derive(Serialize)]
pub struct Status {
    pub devices: Vec<Device>,
    pub pages: Vec<Page>,
    pub default_page: String,
    pub brightness: u8,
    pub auto_paginate: bool,
    pub display_off: bool,
    pub pedal: BTreeMap<String, BTreeMap<String, String>>,
    pub services: BTreeMap<String, String>,
    pub history: state::Flags,
}

#[derive(Default, Deserialize)]
struct Config {
    #[serde(default)]
    deck: DeckConfig,
    #[serde(default)]
    pedal: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
#[serde(default)]
struct DeckConfig {
    brightness: u8,
    default_page: String,
    auto_paginate: bool,
    #[serde(default)]
    display_off: bool,
    #[serde(default)]
    page_order: Vec<String>,
    #[serde(default)]
    pages: BTreeMap<String, PageConfig>,
}

#[derive(Default, Deserialize)]
struct PageConfig {
    #[serde(default)]
    buttons: Vec<ButtonConfig>,
}

#[derive(Default, Deserialize)]
struct ButtonConfig {
    #[serde(default = "no_index")]
    index: i64,
    #[serde(default)]
    label: String,
    #[serde(default)]
    glyph: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    bg: String,
    #[serde(default)]
    fg: String,
    #[serde(default)]
    action: String,
}

fn no_index() -> i64 {
    -1
}

// Mirrors streamdeck-ctl's own defaults; diverging here made the panel draw
// blank keys where the device drew page arrows.
impl Default for DeckConfig {
    fn default() -> Self {
        Self {
            brightness: 70,
            default_page: "main".to_owned(),
            auto_paginate: true,
            display_off: false,
            page_order: Vec::new(),
            pages: BTreeMap::new(),
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("streamdeck-ctl/config.toml")
}

fn preview_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("elgato-panel/deck")
}

/// Re-renders the key previews when the config or the export settings changed.
fn refresh_previews(cols: u8, rows: u8) {
    let dir = preview_dir();
    let stamp = dir.join(".stamp");
    let keys = (cols * rows).to_string();
    let settings = format!("keys={keys} radius={PREVIEW_RADIUS}");
    let config_time = fs::metadata(config_path()).and_then(|m| m.modified()).ok();
    let stamp_time = fs::metadata(&stamp).and_then(|m| m.modified()).ok();
    let unchanged = fs::read_to_string(&stamp).is_ok_and(|s| s == settings);
    if let (Some(config_time), Some(stamp_time)) = (config_time, stamp_time)
        && stamp_time >= config_time
        && unchanged
    {
        return;
    }
    let _ = fs::create_dir_all(&dir);
    sh::run(&[
        "streamdeck-ctl",
        "deck",
        "export",
        "--out",
        &dir.to_string_lossy(),
        "--keys",
        &keys,
        "--radius",
        PREVIEW_RADIUS,
    ]);
    let _ = fs::write(&stamp, &settings);
}

const PREVIEW_RADIUS: &str = "9";

fn devices(listing: &str) -> Vec<Device> {
    listing
        .lines()
        .filter_map(|line| {
            let (kind, serial) = line.split_once("  serial=").unwrap_or((line, ""));
            let kind = kind.trim();
            if kind.is_empty() {
                return None;
            }
            let (cols, rows) = layout(kind);
            Some(Device {
                pedal: kind == "Pedal",
                kind: kind.to_owned(),
                serial: serial.trim().to_owned(),
                cols,
                rows,
            })
        })
        .collect()
}

fn pedal_bindings(pedal: &BTreeMap<String, toml::Value>) -> BTreeMap<String, BTreeMap<String, String>> {
    POSITIONS
        .iter()
        .map(|pos| {
            let table = pedal.get(*pos).and_then(toml::Value::as_table);
            let binds = GESTURES
                .iter()
                .map(|gesture| {
                    let value = table
                        .and_then(|t| t.get(*gesture))
                        .and_then(toml::Value::as_str)
                        .unwrap_or("");
                    let value = if value == "noop" { "" } else { value };
                    ((*gesture).to_owned(), value.to_owned())
                })
                .collect();
            ((*pos).to_owned(), binds)
        })
        .collect()
}

fn ordered_pages(cfg: &DeckConfig) -> Vec<String> {
    if cfg.page_order.is_empty() {
        return cfg.pages.keys().cloned().collect();
    }
    cfg.page_order
        .iter()
        .filter(|name| cfg.pages.contains_key(*name))
        .cloned()
        .collect()
}

/// Mirrors streamdeck-ctl: with auto-pagination on, key 10 walks to the previous
/// page and key 14 to the next, unless a configured button already claims them.
fn neighbours(cfg: &DeckConfig, page: &str) -> (Option<String>, Option<String>) {
    if !cfg.auto_paginate {
        return (None, None);
    }
    let order = ordered_pages(cfg);
    let Some(at) = order.iter().position(|name| name == page) else {
        return (None, None);
    };
    let prev = at.checked_sub(1).map(|i| order[i].clone());
    let next = order.get(at + 1).cloned();
    (prev, next)
}

fn keys(cfg: &DeckConfig, name: &str, page: &PageConfig, count: u8, dir: &Path) -> Vec<Key> {
    let configured: BTreeMap<i64, &ButtonConfig> = page.buttons.iter().map(|b| (b.index, b)).collect();
    let (prev, next) = neighbours(cfg, name);
    (0..count)
        .map(|index| {
            let preview = dir.join(name).join(format!("{index}.png"));
            let preview = if preview.exists() {
                preview.to_string_lossy().into_owned()
            } else {
                String::new()
            };
            if let Some(button) = configured.get(&(index as i64)) {
                let icon = Path::new(&button.icon)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                return Key {
                    index,
                    kind: "button".to_owned(),
                    preview,
                    label: button.label.clone(),
                    glyph: button.glyph.clone(),
                    icon,
                    bg: button.bg.clone(),
                    fg: button.fg.clone(),
                    action: button.action.clone(),
                    target: String::new(),
                };
            }
            let target = match index {
                PREV_INDEX => prev.clone(),
                NEXT_INDEX => next.clone(),
                _ => None,
            };
            match target {
                Some(target) => Key {
                    index,
                    kind: "page".to_owned(),
                    preview,
                    label: target.clone(),
                    glyph: String::new(),
                    icon: String::new(),
                    bg: String::new(),
                    fg: String::new(),
                    action: format!("page:{target}"),
                    target,
                },
                None => Key {
                    index,
                    kind: "blank".to_owned(),
                    preview,
                    label: String::new(),
                    glyph: String::new(),
                    icon: String::new(),
                    bg: String::new(),
                    fg: String::new(),
                    action: String::new(),
                    target: String::new(),
                },
            }
        })
        .collect()
}

fn pages(cfg: &DeckConfig, count: u8) -> Vec<Page> {
    let dir = preview_dir();
    let mut pages: Vec<Page> = cfg
        .pages
        .iter()
        .map(|(name, page)| Page {
            name: name.clone(),
            keys: keys(cfg, name, page, count, &dir),
        })
        .collect();
    let rank = |name: &String| {
        cfg.page_order
            .iter()
            .position(|n| n == name)
            .unwrap_or(cfg.page_order.len())
    };
    pages.sort_by_key(|p| rank(&p.name));
    pages
}

/// A remembered configuration. Brightness and display power are left out of the
/// comparison so dragging the brightness slider cannot evict real key edits from
/// the eleven slots, while the stored text still restores them.
#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub text: String,
    key: String,
}

impl Snapshot {
    pub fn new(text: String) -> Self {
        let key = text
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("brightness") && !trimmed.starts_with("display_off")
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self { text, key }
    }
}

impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

pub fn read_config_text() -> String {
    fs::read_to_string(config_path()).unwrap_or_default()
}

pub fn status() -> Status {
    let probes: Vec<Vec<String>> = std::iter::once(vec!["streamdeck-ctl".to_owned(), "ls".to_owned()])
        .chain(SERVICES.iter().map(|s| {
            vec![
                "systemctl".to_owned(),
                "--user".to_owned(),
                "is-active".to_owned(),
                (*s).to_owned(),
            ]
        }))
        .collect();
    let out = sh::run_all(&probes);
    let devices = devices(&out[0]);
    let text = read_config_text();
    let cfg: Config = toml::from_str(&text).unwrap_or_default();

    let (cols, rows) = devices
        .iter()
        .find(|d| !d.pedal)
        .map(|d| (d.cols, d.rows))
        .unwrap_or((5, 3));
    refresh_previews(cols, rows);

    let mut history: History<Snapshot> = History::load(DECK_HISTORY);
    if !text.is_empty() {
        history.fold(DECK_HISTORY, Snapshot::new(text));
    }

    Status {
        pages: pages(&cfg.deck, cols * rows),
        default_page: cfg.deck.default_page.clone(),
        brightness: cfg.deck.brightness,
        auto_paginate: cfg.deck.auto_paginate,
        display_off: cfg.deck.display_off,
        pedal: pedal_bindings(&cfg.pedal),
        services: SERVICES
            .iter()
            .enumerate()
            .map(|(i, s)| ((*s).to_owned(), out[i + 1].trim().to_owned()))
            .collect(),
        history: history.flags(),
        devices,
    }
}

/// Restores a remembered config and restarts both daemons so the device follows.
pub fn travel(step: i64) {
    let history: History<Snapshot> = History::load(DECK_HISTORY);
    let Some((pos, snapshot)) = history.seek(step) else {
        return;
    };
    let text = &snapshot.text;
    let path = config_path();
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    if fs::write(&tmp, text).is_err() || fs::rename(&tmp, &path).is_err() {
        return;
    }
    history.commit_pos(DECK_HISTORY, pos);
    sh::run_all(&[
        vec!["streamdeck-ctl".to_owned(), "deck".to_owned(), "reload".to_owned()],
        vec!["streamdeck-ctl".to_owned(), "pedal".to_owned(), "reload".to_owned()],
    ]);
}
