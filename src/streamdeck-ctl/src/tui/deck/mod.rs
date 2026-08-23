mod handlers;
mod page_select;
mod view;

use crate::action::ActionKind;
use crate::config::Config;
use ratatui::widgets::TableState;

pub use crate::action::{join as join_action, split as parse_action};
pub use handlers::handle_key;
pub use view::draw;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    EditFieldPicker,
    EditFieldInput,
    EditActionKind,
    EditActionInput,
    PageAddInput,
    PageRemoveConfirm,
    BrightnessInput,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Label,
    Glyph,
    Icon,
    Bg,
    Fg,
}

impl Field {
    pub fn label(&self) -> &'static str {
        match self {
            Field::Label => "label",
            Field::Glyph => "glyph",
            Field::Icon => "icon path",
            Field::Bg => "bg color",
            Field::Fg => "fg color",
        }
    }
}

/// Fallback when no deck is attached to ask; a Mk2's fifteen keys.
pub const ROWS_PER_PAGE: u8 = 15;

/// Key count of the attached deck, so the table neither hides keys on an XL nor
/// offers keys a Mini does not have.
pub fn rows_per_page() -> u8 {
    crate::device::list_decks()
        .ok()
        .and_then(|d| d.first().map(|(kind, _)| kind.key_count()))
        .unwrap_or(ROWS_PER_PAGE)
}

pub struct DeckView {
    pub current_page: String,
    pub table: TableState,
    pub mode: Mode,
    pub edit_field: Field,
    pub edit_action_kind: ActionKind,
    pub edit_buffer: String,
    pub page_names: Vec<String>,
}

impl DeckView {
    pub fn new(cfg: &Config) -> Self {
        let mut t = TableState::default();
        t.select(Some(0));
        let names: Vec<String> = ordered_pages_with_unordered_appended(cfg);
        let current = if cfg.deck.pages.contains_key(&cfg.deck.default_page) {
            cfg.deck.default_page.clone()
        } else if let Some(first) = names.first() {
            first.clone()
        } else {
            "main".into()
        };
        Self {
            current_page: current,
            table: t,
            mode: Mode::Normal,
            edit_field: Field::Label,
            edit_action_kind: ActionKind::Key,
            edit_buffer: String::new(),
            page_names: names,
        }
    }

    pub fn modal_open(&self) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    pub fn modal_title(&self) -> String {
        match self.mode {
            Mode::EditFieldPicker
            | Mode::EditFieldInput
            | Mode::EditActionKind
            | Mode::EditActionInput => format!(
                " edit: {} #{:02} ",
                self.current_page,
                self.table.selected().unwrap_or(0)
            ),
            Mode::PageAddInput => " new page name ".into(),
            Mode::PageRemoveConfirm => format!(" remove page '{}' ", self.current_page),
            Mode::BrightnessInput => " deck brightness (0-100) ".into(),
            Mode::Normal => String::new(),
        }
    }

    pub fn draw_modal(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        view::draw_modal(f, area, self)
    }

    pub fn selected_index(&self) -> u8 {
        self.table.selected().unwrap_or(0) as u8
    }

    pub fn reconcile_after_config_change(&mut self, cfg: &Config) {
        self.page_names = ordered_pages_with_unordered_appended(cfg);
        if !cfg.deck.pages.contains_key(&self.current_page) {
            self.current_page = self
                .page_names
                .first()
                .cloned()
                .unwrap_or_else(|| "main".into());
        }
    }
}

/// Pages in `page_order` first (in that order), then any remaining pages
/// alphabetically. Lets the TUI cycle through ordered pages naturally and
/// still see un-ordered ones at the end.
fn ordered_pages_with_unordered_appended(cfg: &Config) -> Vec<String> {
    let mut out: Vec<String> = cfg.deck.ordered_pages();
    for k in cfg.deck.pages.keys() {
        if !out.contains(k) {
            out.push(k.clone());
        }
    }
    out
}

