use crate::tui::state::App;

pub fn next(app: &mut App) {
    cycle(app, 1);
}

pub fn prev(app: &mut App) {
    cycle(app, -1);
}

fn cycle(app: &mut App, delta: i32) {
    let names = &app.deck.page_names;
    if names.is_empty() {
        return;
    }
    let cur = names
        .iter()
        .position(|n| n == &app.deck.current_page)
        .unwrap_or(0) as i32;
    let next = (cur + delta).rem_euclid(names.len() as i32) as usize;
    app.deck.current_page = names[next].clone();
}
