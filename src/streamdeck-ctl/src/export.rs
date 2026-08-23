use crate::config::{Button, Config};
use crate::device;
use crate::render::Renderer;
use anyhow::{Context, Result};
use image::{DynamicImage, RgbaImage};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const FALLBACK_SIZE: u32 = 72;
const FALLBACK_KEYS: u8 = 15;

/// Writes every key of every page as `<out>/<page>/<index>.png`, using the same
/// renderer the daemon pushes to the device.
pub fn run(
    cfg: &Config,
    out: &Path,
    page: Option<String>,
    size: Option<u32>,
    keys: Option<u8>,
    radius: Option<f32>,
) -> Result<()> {
    let device = device::deck::open_first().ok();
    let size = size
        .or_else(|| device.as_ref().map(|d| d.kind.key_image_format().size.0 as u32))
        .unwrap_or(FALLBACK_SIZE);
    let key_count = keys
        .or_else(|| device.as_ref().map(|d| d.kind.key_count()))
        .unwrap_or(FALLBACK_KEYS);

    let renderer = Renderer::new(&cfg.deck, size)?;
    let pages: Vec<String> = match page {
        Some(name) => vec![name],
        None => cfg.deck.pages.keys().cloned().collect(),
    };

    for name in pages {
        let Some(page) = cfg.deck.pages.get(&name) else {
            anyhow::bail!("page not found: {name}");
        };
        let dir = out.join(&name);
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let by_index: HashMap<u8, &Button> = page.buttons.iter().map(|b| (b.index, b)).collect();
        for index in 0..key_count {
            let image = match by_index.get(&index) {
                Some(btn) => renderer.render_button(btn)?,
                None => match cfg.deck.synthetic_button(&name, index) {
                    Some(synth) => renderer.render_button(&synth)?,
                    None => renderer.blank(),
                },
            };
            let path = dir.join(format!("{index}.png"));
            match radius {
                Some(radius) if radius > 0.0 => round_corners(&image, radius)
                    .save(&path)
                    .with_context(|| format!("write {}", path.display()))?,
                _ => image
                    .save(&path)
                    .with_context(|| format!("write {}", path.display()))?,
            }
        }
    }
    Ok(())
}

/// Feathers the key corners to transparent, for previews drawn on a surface
/// rather than pushed to the device's square LCD.
fn round_corners(image: &DynamicImage, radius: f32) -> DynamicImage {
    let source = image.to_rgba8();
    let (width, height) = source.dimensions();
    let radius = radius.min(width.min(height) as f32 / 2.0);
    let mut out = RgbaImage::from_raw(width, height, source.into_raw()).expect("same dimensions");
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = (radius - px).max(px - (width as f32 - radius)).max(0.0);
            let dy = (radius - py).max(py - (height as f32 - radius)).max(0.0);
            if dx <= 0.0 || dy <= 0.0 {
                continue;
            }
            let coverage = (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            let pixel = out.get_pixel_mut(x, y);
            pixel.0[3] = (pixel.0[3] as f32 * coverage).round() as u8;
        }
    }
    DynamicImage::ImageRgba8(out)
}
