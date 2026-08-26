use crate::sh;
use crate::state::{self, History, DECK_HISTORY};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SERVICES: [&str; 2] = ["streamdeck-ctl.service", "streamdeck-ctl-deck.service"];
const GESTURES: [&str; 3] = ["tap", "long", "double"];
const POSITIONS: [&str; 3] = ["left", "center", "right"];
/// Reported by `streamdeck-ctl ls --json`, which reads it from the device
/// library, so every model lays out correctly without a table here.
#[derive(Clone, Serialize, Deserialize)]
pub struct Device {
    pub kind: String,
    #[serde(default)]
    pub name: String,
    pub serial: String,
    #[serde(default)]
    pub keys: u8,
    pub cols: u8,
    pub rows: u8,
    #[serde(default)]
    pub encoders: u8,
    #[serde(default)]
    pub visual: bool,
    pub pedal: bool,
}

/// The two ends of the bottom row, matching streamdeck-ctl's own placement.
fn pagination_indices(keys: u8, cols: u8) -> Option<(u8, u8)> {
    if keys == 0 || cols == 0 || keys < cols {
        return None;
    }
    Some((keys - cols, keys - 1))
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
pub struct Binding {
    pub action: String,
    /// What the gesture will actually do, in words.
    pub label: String,
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
    pub pedal: BTreeMap<String, BTreeMap<String, Binding>>,
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
        .map(|c| c.join("omgato-panel/deck"))
        .unwrap_or_else(|| crate::state::dir().join("cache/deck"))
}

/// Re-renders the key previews when the config or the export settings changed.
fn refresh_previews(key_count: u8, cols: u8) {
    let dir = preview_dir();
    let stamp = dir.join(".stamp");
    let want = format!(
        "keys={key_count} cols={cols} radius={PREVIEW_RADIUS} config={}",
        config_revision()
    );
    if fs::read_to_string(&stamp).is_ok_and(|have| have == want) {
        return;
    }
    let _ = fs::create_dir_all(&dir);
    let exported = sh::succeeded(&[
        "streamdeck-ctl",
        "deck",
        "export",
        "--out",
        &dir.to_string_lossy(),
        "--keys",
        &key_count.to_string(),
        "--radius",
        PREVIEW_RADIUS,
    ]);
    if exported {
        let tmp = stamp.with_extension(format!("{}.tmp", std::process::id()));
        if let Ok(mut file) = fs::OpenOptions::new().create_new(true).write(true).open(&tmp)
            && file.write_all(want.as_bytes()).is_ok()
        {
            file.sync_all().ok();
            drop(file);
            let _ = fs::rename(&tmp, &stamp);
        }
    }
}

/// Identifies the configuration the previews were built from.
fn config_revision() -> String {
    let Ok(meta) = fs::metadata(config_path()) else {
        return "none".to_owned();
    };
    let stamp = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{stamp}-{}", meta.len())
}

const PREVIEW_RADIUS: &str = "9";

fn devices(listing: &str) -> Vec<Device> {
    serde_json::from_str(listing).unwrap_or_default()
}

/// Turns an action into the name of the thing it starts.
fn describe(action: &str) -> String {
    let action = action.trim();
    if action.is_empty() || action == "noop" {
        return String::new();
    }
    if let Some(key) = action.strip_prefix("key:") {
        return key.trim_start_matches("KEY_").replace('_', " ");
    }
    if let Some(page) = action.strip_prefix("page:") {
        return format!("Page {page}");
    }
    let command = action.strip_prefix("exec:").unwrap_or(action).trim();
    let mut parts = command.split_whitespace();
    let Some(binary) = parts.next() else {
        return String::new();
    };
    let binary = binary.rsplit('/').next().unwrap_or(binary);
    if let Some(url) = parts.clone().find(|p| p.starts_with("http")) {
        return url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .split('/')
            .next()
            .unwrap_or(url)
            .to_owned();
    }
    if let Some(rest) = binary.strip_prefix("omarchy-launch-") {
        let mut name = rest.replace('-', " ");
        if let Some(first) = name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        return name;
    }
    binary.to_owned()
}

fn pedal_bindings(pedal: &BTreeMap<String, toml::Value>) -> BTreeMap<String, BTreeMap<String, Binding>> {
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
                    (
                        (*gesture).to_owned(),
                        Binding { action: value.to_owned(), label: describe(value) },
                    )
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

fn keys(cfg: &DeckConfig, name: &str, page: &PageConfig, count: u8, cols: u8, dir: &Path) -> Vec<Key> {
    let configured: BTreeMap<i64, &ButtonConfig> = page.buttons.iter().map(|b| (b.index, b)).collect();
    let (prev, next) = neighbours(cfg, name);
    let pagination = pagination_indices(count, cols);
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
            let target = match pagination {
                Some((prev_index, _)) if index == prev_index => prev.clone(),
                Some((_, next_index)) if index == next_index => next.clone(),
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

fn pages(cfg: &DeckConfig, count: u8, cols: u8) -> Vec<Page> {
    let dir = preview_dir();
    let mut pages: Vec<Page> = cfg
        .pages
        .iter()
        .map(|(name, page)| Page {
            name: name.clone(),
            keys: keys(cfg, name, page, count, cols, &dir),
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
    let mut probe = vec!["systemctl".to_owned(), "--user".to_owned(), "is-active".to_owned()];
    probe.extend(SERVICES.iter().map(|s| (*s).to_owned()));
    let out = sh::run_all(&[
        vec!["streamdeck-ctl".to_owned(), "ls".to_owned(), "--json".to_owned()],
        probe,
    ]);
    let devices = devices(&out[0]);
    let text = read_config_text();
    let cfg: Config = toml::from_str(&text).unwrap_or_default();

    let deck_device = devices.iter().find(|d| !d.pedal);
    let cols = deck_device.map(|d| d.cols).unwrap_or(5);
    let keys = deck_device.map(|d| d.keys).unwrap_or(15);
    refresh_previews(keys, cols);

    let mut history: History<Snapshot> = History::load(DECK_HISTORY);
    if !text.is_empty() {
        history.fold(DECK_HISTORY, Snapshot::new(text));
    }

    Status {
        pages: pages(&cfg.deck, keys, cols),
        default_page: cfg.deck.default_page.clone(),
        brightness: cfg.deck.brightness,
        auto_paginate: cfg.deck.auto_paginate,
        display_off: cfg.deck.display_off,
        pedal: pedal_bindings(&cfg.pedal),
        services: SERVICES
            .iter()
            .zip(out[1].lines().chain(std::iter::repeat("")))
            .map(|(unit, state)| ((*unit).to_owned(), state.trim().to_owned()))
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
    let written = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all().ok();
            Ok(())
        });
    if written.is_err() || fs::rename(&tmp, &path).is_err() {
        return;
    }
    history.commit_pos(DECK_HISTORY, pos);
    sh::run_all(&[
        vec!["streamdeck-ctl".to_owned(), "deck".to_owned(), "reload".to_owned()],
        vec!["streamdeck-ctl".to_owned(), "pedal".to_owned(), "reload".to_owned()],
    ]);
}
