//! Colour the deck's pages from the current Omarchy theme.
//!
//! The panel already retints itself when the theme changes; the keys are the
//! one surface that does not. `deck theme` closes that gap by resolving the
//! theme's palette and giving each page a background from it.
//!
//! Picking colours is the interesting part. Reading four fixed palette keys
//! (blue, green, magenta, yellow) fails on any theme whose palette is warm or
//! monochrome throughout, where all four land on nearly the same colour. So
//! consider the whole palette and choose the entries that sit furthest apart
//! perceptually, falling back to shading one accent only when the palette
//! genuinely holds nothing else.

use crate::config::{Config, DeckConfig};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Palette keys worth considering as a page colour, most characteristic first.
/// The accent seeds the selection, so the first page keeps the colour the
/// theme leads with.
const CANDIDATE_KEYS: &[&str] = &[
    "accent", "red", "green", "yellow", "blue", "magenta", "cyan", "orange",
    "purple", "brown", "bright_red", "bright_green", "bright_yellow",
    "bright_blue", "bright_magenta", "bright_cyan", "muted",
];

/// Below this perceptual distance between the resulting tints, the pages are
/// not tellable apart and shading an accent reads better than pretending.
const MIN_SEPARATION: f64 = 5.0;

/// A tint is eased back toward the background until the label clears this
/// contrast ratio against it.
const MIN_CONTRAST: f64 = 3.0;

/// A candidate this close to the background cannot tint it at any strength.
const MIN_FROM_BACKGROUND: f64 = 12.0;

/// Multiples of `strength` bounding the accent-only fallback.
const LIGHTNESS_MIN: f64 = 0.474;
const LIGHTNESS_MAX: f64 = 1.789;

pub type Rgb = [f64; 3];

pub struct Palette(BTreeMap<String, String>);

impl Palette {
    #[cfg(test)]
    fn of(pairs: &[(&str, &str)]) -> Self {
        Self(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    }

    fn get(&self, key: &str) -> Option<Rgb> {
        self.0.get(key).and_then(|v| parse_hex(v))
    }

    fn first(&self, keys: &[&str], fallback: Rgb) -> Rgb {
        keys.iter().find_map(|k| self.get(k)).unwrap_or(fallback)
    }
}

/// Resolve the palette through `omarchy-theme-color`, which applies the same
/// alias and fallback cascade every other Omarchy consumer sees.
pub fn load(colors_file: Option<&Path>) -> Result<Palette> {
    let mut cmd = Command::new("omarchy-theme-color");
    if let Some(path) = colors_file {
        cmd.arg("--file").arg(path);
    }
    let out = cmd
        .arg("--all")
        .output()
        .context("running omarchy-theme-color (is Omarchy installed?)")?;
    if !out.status.success() {
        anyhow::bail!("omarchy-theme-color exited {}", out.status);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(Palette(
        text.lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .collect(),
    ))
}

pub fn parse_hex(value: &str) -> Option<Rgb> {
    let digits = value.trim().strip_prefix('#')?;
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([byte(0)? as f64, byte(2)? as f64, byte(4)? as f64])
}

pub fn to_hex(c: Rgb) -> String {
    let clamp = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", clamp(c[0]), clamp(c[1]), clamp(c[2]))
}

/// `t` of `a` laid over `b`.
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    [
        a[0] * t + b[0] * (1.0 - t),
        a[1] * t + b[1] * (1.0 - t),
        a[2] * t + b[2] * (1.0 - t),
    ]
}

fn linearise(v: f64) -> f64 {
    let v = v / 255.0;
    if v <= 0.03928 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn luminance(c: Rgb) -> f64 {
    let (r, g, b) = (linearise(c[0]), linearise(c[1]), linearise(c[2]));
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG relative contrast ratio, 1.0 (identical) to 21.0 (black on white).
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

fn to_lab(c: Rgb) -> [f64; 3] {
    let (r, g, b) = (linearise(c[0]), linearise(c[1]), linearise(c[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIE76 colour difference. Roughly: 2.3 is just noticeable, 10 is obvious.
pub fn delta_e(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (to_lab(a), to_lab(b));
    ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}

/// Greedy farthest-point selection: after the seed, repeatedly take whichever
/// candidate is furthest from everything picked so far.
pub fn pick_distinct(candidates: &[Rgb], seed: Rgb, want: usize) -> Vec<Rgb> {
    if candidates.is_empty() {
        return vec![seed; want];
    }
    let nearest_to_seed = candidates
        .iter()
        .copied()
        .min_by(|a, b| {
            delta_e(*a, seed)
                .partial_cmp(&delta_e(*b, seed))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(seed);
    let mut chosen = vec![nearest_to_seed];
    while chosen.len() < want {
        let next = candidates
            .iter()
            .copied()
            .filter(|c| !chosen.iter().any(|p| p == c))
            .max_by(|a, b| {
                let score = |c: &Rgb| {
                    chosen
                        .iter()
                        .map(|p| delta_e(*c, *p))
                        .fold(f64::INFINITY, f64::min)
                };
                score(a)
                    .partial_cmp(&score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        match next {
            Some(c) => chosen.push(c),
            // Fewer distinct colours than pages: reuse, rather than fail.
            None => chosen.push(chosen[chosen.len() % candidates.len()]),
        }
    }
    chosen
}

/// How the pages ended up being told apart, reported so `--dry-run` can say.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Separation {
    /// Distinct colours from the palette.
    Hue,
    /// One accent at increasing strengths, for palettes with nothing else.
    Lightness,
}

pub struct Scheme {
    pub background: String,
    pub foreground: String,
    pub pages: Vec<(String, String)>,
    pub separation: Separation,
    pub closest: f64,
}

pub fn scheme(deck: &DeckConfig, palette: &Palette, strength: f64) -> Scheme {
    let base = palette.first(&["lighter_background", "background"], [26.0, 26.0, 26.0]);
    let fg = palette.first(&["foreground", "fg"], [255.0, 255.0, 255.0]);
    let accent = palette.first(&["accent", "blue"], [128.0, 128.0, 128.0]);

    let names = deck.ordered_pages();
    let want = names.len().max(1);

    let mut seen: Vec<Rgb> = Vec::new();
    for key in CANDIDATE_KEYS {
        if let Some(c) = palette.get(key)
            && delta_e(c, base) > MIN_FROM_BACKGROUND
            && !seen.iter().any(|s| delta_e(*s, c) < 1.0)
        {
            seen.push(c);
        }
    }

    let mut tints: Vec<Rgb> = pick_distinct(&seen, accent, want)
        .into_iter()
        .map(|c| mix(c, base, strength))
        .collect();

    let closest = closest_pair(&tints);
    let separation = if closest < MIN_SEPARATION {
        // Nothing in the palette separates the pages, so separate them by
        // weight instead: the same accent, laid on progressively thicker.
        let steps = want.max(1);
        tints = (0..steps)
            .map(|i| {
                // Spread evenly between a light wash and a strong one, both
                // scaled by `strength` so the knob still governs the whole
                // range. Tuned so four pages step clearly without the last
                // one becoming a flat block of accent.
                let f = if steps > 1 {
                    i as f64 / (steps - 1) as f64
                } else {
                    0.0
                };
                let alpha = strength * (LIGHTNESS_MIN + f * (LIGHTNESS_MAX - LIGHTNESS_MIN));
                mix(accent, base, alpha.clamp(0.0, 1.0))
            })
            .collect();
        Separation::Lightness
    } else {
        Separation::Hue
    };

    // Keep every label legible: ease a tint back toward the background until
    // it clears the contrast floor against the theme's foreground.
    for tint in &mut tints {
        let mut guard = 0;
        while contrast(*tint, fg) < MIN_CONTRAST && guard < 24 {
            *tint = mix(*tint, base, 0.8);
            guard += 1;
        }
    }

    Scheme {
        background: to_hex(base),
        foreground: to_hex(fg),
        pages: names.into_iter().zip(tints.into_iter().map(to_hex)).collect(),
        separation,
        closest,
    }
}

fn closest_pair(tints: &[Rgb]) -> f64 {
    let mut closest = f64::INFINITY;
    for (i, a) in tints.iter().enumerate() {
        for b in &tints[i + 1..] {
            closest = closest.min(delta_e(*a, *b));
        }
    }
    if closest.is_finite() { closest } else { 0.0 }
}

pub fn apply(cfg: &mut Config, scheme: &Scheme) {
    cfg.deck.bg_color = scheme.background.clone();
    cfg.deck.text_color = scheme.foreground.clone();
    for (name, colour) in &scheme.pages {
        if let Some(page) = cfg.deck.pages.get_mut(name) {
            page.bg = Some(colour.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Page;

    const BLACK: Rgb = [0.0, 0.0, 0.0];
    const WHITE: Rgb = [255.0, 255.0, 255.0];

    fn deck_with(pages: &[&str]) -> DeckConfig {
        let mut deck = DeckConfig::default();
        deck.pages.clear();
        deck.page_order = pages.iter().map(|p| (*p).to_owned()).collect();
        for name in pages {
            deck.pages.insert((*name).to_owned(), Page::default());
        }
        deck
    }

    /// A palette whose colours are all near-identical warm pinks, the case
    /// that made four fixed palette keys useless.
    fn warm_palette() -> Palette {
        Palette::of(&[
            ("background", "#000000"),
            ("lighter_background", "#1a1a1a"),
            ("foreground", "#e7e6e5"),
            ("accent", "#ac6380"),
            ("blue", "#ac6380"),
            ("magenta", "#e98897"),
            ("purple", "#e98897"),
            ("red", "#c47d75"),
        ])
    }

    fn varied_palette() -> Palette {
        Palette::of(&[
            ("lighter_background", "#24283b"),
            ("foreground", "#a9b1d6"),
            ("accent", "#7aa2f7"),
            ("blue", "#7aa2f7"),
            ("green", "#9ece6a"),
            ("magenta", "#ad8ee6"),
            ("yellow", "#e0af68"),
            ("red", "#f7768e"),
        ])
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(parse_hex("#1a2b3c"), Some([26.0, 43.0, 60.0]));
        assert_eq!(to_hex([26.0, 43.0, 60.0]), "#1a2b3c");
        assert_eq!(parse_hex("1a2b3c"), None, "a bare value is not a colour");
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#gggggg"), None);
    }

    #[test]
    fn to_hex_clamps_rather_than_wrapping() {
        assert_eq!(to_hex([-5.0, 300.0, 128.4]), "#00ff80");
    }

    #[test]
    fn mixing_interpolates_between_the_two_colours() {
        assert_eq!(mix(WHITE, BLACK, 1.0), WHITE);
        assert_eq!(mix(WHITE, BLACK, 0.0), BLACK);
        assert_eq!(mix(WHITE, BLACK, 0.5), [127.5, 127.5, 127.5]);
    }

    #[test]
    fn contrast_spans_the_wcag_range() {
        assert!((contrast(BLACK, WHITE) - 21.0).abs() < 0.01);
        assert!((contrast(WHITE, WHITE) - 1.0).abs() < 0.01);
        assert!((contrast(BLACK, WHITE) - contrast(WHITE, BLACK)).abs() < 1e-9);
    }

    #[test]
    fn delta_e_is_zero_for_a_colour_against_itself() {
        assert!(delta_e([120.0, 30.0, 60.0], [120.0, 30.0, 60.0]) < 1e-9);
        assert!(delta_e(BLACK, WHITE) > 90.0);
    }

    #[test]
    fn selection_prefers_colours_that_are_far_apart() {
        let candidates = [
            [255.0, 0.0, 0.0],
            [250.0, 5.0, 5.0], // all but identical to the first
            [0.0, 255.0, 0.0],
            [0.0, 0.0, 255.0],
        ];
        let picked = pick_distinct(&candidates, [255.0, 0.0, 0.0], 3);
        assert_eq!(picked.len(), 3);
        let closest = closest_pair(&picked);
        assert!(closest > 50.0, "picked near-duplicates: {picked:?} ({closest})");
    }

    #[test]
    fn selection_reuses_colours_when_a_palette_is_short() {
        let picked = pick_distinct(&[[255.0, 0.0, 0.0], [0.0, 0.0, 255.0]], BLACK, 4);
        assert_eq!(picked.len(), 4, "must always yield one colour per page");
    }

    #[test]
    fn selection_survives_an_empty_palette() {
        let picked = pick_distinct(&[], [10.0, 20.0, 30.0], 4);
        assert_eq!(picked, vec![[10.0, 20.0, 30.0]; 4]);
    }

    #[test]
    fn a_varied_palette_separates_the_pages_by_hue() {
        let deck = deck_with(&["one", "two", "three", "four"]);
        let scheme = scheme(&deck, &varied_palette(), 0.38);
        assert_eq!(scheme.separation, Separation::Hue);
        assert_eq!(scheme.pages.len(), 4);
    }

    /// The whole point: a palette with nothing distinct in it should shade one
    /// accent rather than produce four colours that look the same.
    #[test]
    fn a_warm_palette_falls_back_to_shading_the_accent() {
        let deck = deck_with(&["one", "two", "three", "four"]);
        let scheme = scheme(&deck, &warm_palette(), 0.38);
        assert_eq!(scheme.separation, Separation::Lightness);
        let tints: Vec<Rgb> = scheme
            .pages
            .iter()
            .map(|(_, c)| parse_hex(c).expect("a colour"))
            .collect();
        assert!(
            closest_pair(&tints) > 3.0,
            "the shaded steps still have to be tellable apart: {tints:?}"
        );
    }

    #[test]
    fn every_page_stays_readable_against_the_foreground() {
        for palette in [warm_palette(), varied_palette()] {
            let deck = deck_with(&["one", "two", "three", "four"]);
            let scheme = scheme(&deck, &palette, 0.9);
            let fg = parse_hex(&scheme.foreground).expect("a colour");
            for (name, colour) in &scheme.pages {
                let ratio = contrast(parse_hex(colour).expect("a colour"), fg);
                assert!(ratio >= MIN_CONTRAST - 0.01, "{name} at {colour}: {ratio}");
            }
        }
    }

    #[test]
    fn a_colour_is_produced_for_every_page_however_many_there_are() {
        for count in [1usize, 2, 4, 7] {
            let names: Vec<String> = (0..count).map(|i| format!("p{i}")).collect();
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let deck = deck_with(&refs);
            let scheme = scheme(&deck, &varied_palette(), 0.38);
            assert_eq!(scheme.pages.len(), count, "{count} pages");
        }
    }

    #[test]
    fn applying_writes_the_globals_and_every_page() {
        let mut cfg = Config {
            deck: deck_with(&["one", "two"]),
            ..Default::default()
        };
        let scheme = scheme(&cfg.deck.clone(), &varied_palette(), 0.38);
        apply(&mut cfg, &scheme);
        assert_eq!(cfg.deck.bg_color, scheme.background);
        assert_eq!(cfg.deck.text_color, scheme.foreground);
        assert!(cfg.deck.pages["one"].bg.is_some());
        assert!(cfg.deck.pages["two"].bg.is_some());
    }
}
