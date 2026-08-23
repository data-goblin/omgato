use crate::config::{self, Config};
use crate::{device, units};
use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Pedal,
    Deck,
}

impl Tab {
    pub fn label(&self) -> &'static str {
        match self {
            Tab::Pedal => "Pedal",
            Tab::Deck => "Deck",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Tab::Pedal => Tab::Deck,
            Tab::Deck => Tab::Pedal,
        }
    }
}

pub struct Connectivity {
    pub pedal_connected: bool,
    pub deck_connected: bool,
    pub pedal_active: bool,
    pub deck_active: bool,
}

pub struct App {
    pub cfg: Config,
    pub conn: Connectivity,
    pub tab: Tab,
    pub pedal: super::pedal::PedalView,
    pub deck: super::deck::DeckView,
    pub msg: Option<(String, Color)>,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let cfg = config::load()?;
        Ok(Self {
            tab: Tab::Pedal,
            pedal: super::pedal::PedalView::new(),
            deck: super::deck::DeckView::new(&cfg),
            cfg,
            conn: Connectivity {
                pedal_connected: false,
                deck_connected: false,
                pedal_active: false,
                deck_active: false,
            },
            msg: None,
        })
    }

    pub fn refresh(&mut self) {
        self.cfg = config::load().unwrap_or_else(|_| self.cfg.clone());
        self.conn = Connectivity {
            pedal_connected: device::list_pedals().map(|v| !v.is_empty()).unwrap_or(false),
            deck_connected: device::list_decks().map(|v| !v.is_empty()).unwrap_or(false),
            pedal_active: units::service_active(),
            deck_active: units::deck_service_active(),
        };
        self.deck.reconcile_after_config_change(&self.cfg);
    }

    pub fn flash(&mut self, msg: impl Into<String>, color: Color) {
        self.msg = Some((msg.into(), color));
    }

    pub fn clear_msg(&mut self) {
        self.msg = None;
    }

    pub fn is_idle_for_refresh(&self) -> bool {
        !self.modal_open()
    }

    pub fn modal_open(&self) -> bool {
        match self.tab {
            Tab::Pedal => self.pedal.modal_open(),
            Tab::Deck => self.deck.modal_open(),
        }
    }
}
