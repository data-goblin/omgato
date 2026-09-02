use crate::action::{self, Action};
use crate::config::{Button, Page};
use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, HashMap};

pub struct ParsedButton {
    pub btn: Button,
    pub action: Action,
}

pub struct ParsedPage {
    pub bg: Option<String>,
    pub buttons: Vec<ParsedButton>,
}

pub fn parse_all(pages: &BTreeMap<String, Page>) -> Result<HashMap<String, ParsedPage>> {
    let mut out = HashMap::new();
    for (name, page) in pages {
        let mut bs = Vec::new();
        for b in &page.buttons {
            let action = action::parse(&b.action)
                .map_err(|e| anyhow!("page='{}' button index={}: {}", name, b.index, e))?;
            bs.push(ParsedButton {
                btn: b.clone(),
                action,
            });
        }
        out.insert(
            name.clone(),
            ParsedPage {
                bg: page.bg.clone(),
                buttons: bs,
            },
        );
    }
    Ok(out)
}
