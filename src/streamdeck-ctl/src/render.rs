use crate::config::{Button, DeckConfig};
use ab_glyph::{point, Font, FontVec, PxScale};
use anyhow::{Context, Result};
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
use std::fs;

pub struct Renderer {
    pub size: u32,
    label_font: FontVec,
    glyph_font: FontVec,
    default_bg: Rgb<u8>,
    default_fg: Rgb<u8>,
}

impl Renderer {
    pub fn new(cfg: &DeckConfig, size: u32) -> Result<Self> {
        let label_bytes = read_font(&cfg.font_label, "sans-serif")
            .with_context(|| format!("read label font: {}", cfg.font_label))?;
        let glyph_bytes = read_font(&cfg.font_glyph, "Symbols Nerd Font")
            .with_context(|| format!("read glyph font: {}", cfg.font_glyph))?;
        let label_font =
            FontVec::try_from_vec(label_bytes).map_err(|e| anyhow::anyhow!("label font: {e}"))?;
        let glyph_font =
            FontVec::try_from_vec(glyph_bytes).map_err(|e| anyhow::anyhow!("glyph font: {e}"))?;
        Ok(Self {
            size,
            label_font,
            glyph_font,
            default_bg: parse_hex(&cfg.bg_color)?,
            default_fg: parse_hex(&cfg.text_color)?,
        })
    }

    pub fn render_button(&self, btn: &Button) -> Result<DynamicImage> {
        let s = self.size;
        let bg = btn.bg.as_deref().map(parse_hex).transpose()?.unwrap_or(self.default_bg);
        let fg = btn.fg.as_deref().map(parse_hex).transpose()?.unwrap_or(self.default_fg);

        let mut img: RgbImage = ImageBuffer::from_pixel(s, s, bg);

        if let Some(icon_path) = &btn.icon {
            self.draw_icon(&mut img, icon_path, bg);
        } else if let Some(glyph) = &btn.glyph {
            self.draw_glyph(&mut img, glyph, fg);
        }

        if !btn.label.is_empty() {
            self.draw_label(&mut img, &btn.label, fg);
        }

        Ok(DynamicImage::ImageRgb8(img))
    }

    pub fn blank(&self) -> DynamicImage {
        let img: RgbImage = ImageBuffer::from_pixel(self.size, self.size, self.default_bg);
        DynamicImage::ImageRgb8(img)
    }

    fn draw_icon(&self, img: &mut RgbImage, icon_path: &str, _bg: Rgb<u8>) {
        let Ok(icon) = image::open(icon_path) else {
            eprintln!("streamdeck-ctl: render: failed to open icon {}", icon_path);
            return;
        };
        let s = self.size;
        let target = (s as f32 * 0.55) as u32;
        let x_off = (s - target) / 2 ;
        // Pull up so the icon sits in the upper-third like glyphs do, leaving
        // clearer space for the label band at the bottom.
        let y_off = (s as f32 * 0.10) as u32;
        let resized = icon
            .resize_exact(target, target, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        for (px, py, pixel) in resized.enumerate_pixels() {
            let dst_x = x_off + px;
            let dst_y = y_off + py;
            if dst_x >= s || dst_y >= s {
                continue;
            }
            let [r, g, b, a] = pixel.0;
            if a == 0 {
                continue;
            }
            let af = a as f32 / 255.0;
            let dst = img.get_pixel_mut(dst_x, dst_y);
            for (i, &src_c) in [r, g, b].iter().enumerate() {
                dst.0[i] = ((src_c as f32 * af) + (dst.0[i] as f32 * (1.0 - af))).round() as u8;
            }
        }
    }

    fn draw_glyph(&self, img: &mut RgbImage, glyph: &str, fg: Rgb<u8>) {
        let s = self.size;
        let scale = PxScale::from(s as f32 * 0.55);
        let Some(c) = glyph.chars().next() else { return };
        // Visible bounding box of just this glyph (accounts for left side bearing).
        let g = self.glyph_font.glyph_id(c).with_scale_and_position(scale, point(0.0, 0.0));
        if let Some(outlined) = self.glyph_font.outline_glyph(g) {
            let b = outlined.px_bounds();
            let visible_w = b.max.x - b.min.x;
            let visible_h = b.max.y - b.min.y;
            // x: center the visible glyph horizontally
            let x = ((s as f32 - visible_w) / 2.0 - b.min.x).round() as i32;
            // y: visible top inset equals horizontal inset, so the glyph reads
            // as visually padded equally from top/left/right.
            let target_top = (s as f32 - visible_w) / 2.0;
            let y = (target_top - scale.y - b.min.y).round() as i32;
            draw_text_mut(img, fg, x, y, scale, &self.glyph_font, glyph);
            let _ = visible_h;
            return;
        }
        // Glyph has no outline (e.g. space, missing). Centered fallback via text_size.
        let (gw, _) = text_size(scale, &self.glyph_font, glyph);
        let x = ((s as i32 - gw as i32) / 2).max(0);
        let y = ((s as f32 * 0.08) as i32).max(0);
        draw_text_mut(img, fg, x, y, scale, &self.glyph_font, glyph);
    }

    /// Fits the label to the key: shrinks the type, then wraps to two lines,
    /// and ellipsizes only when neither is enough.
    fn draw_label(&self, img: &mut RgbImage, label: &str, fg: Rgb<u8>) {
        let s = self.size as f32;
        let budget = s * LABEL_WIDTH_BUDGET;
        let mut steps = 0;
        let lines = loop {
            let scale = PxScale::from(s * (LABEL_SCALE_MAX - steps as f32 * LABEL_SCALE_STEP));
            if self.line_width(scale, label) <= budget {
                break vec![(scale, label.to_string())];
            }
            if let Some((head, tail)) = split_label(label)
                && self.line_width(scale, &head).max(self.line_width(scale, &tail)) <= budget {
                    break vec![(scale, head), (scale, tail)];
                }
            steps += 1;
            if s * (LABEL_SCALE_MAX - steps as f32 * LABEL_SCALE_STEP) < s * LABEL_SCALE_MIN {
                let scale = PxScale::from(s * LABEL_SCALE_MIN);
                break vec![(scale, self.ellipsize(scale, label, budget))];
            }
        };

        let (_, first_h) = text_size(lines[0].0, &self.label_font, "Ag");
        let line_step = first_h as f32 * 1.12;
        let block_h = line_step * lines.len() as f32;
        let baseline = s - block_h - s / 14.0;
        for (i, (scale, text)) in lines.iter().enumerate() {
            let width = self.line_width(*scale, text);
            let x = ((s - width) / 2.0).max(0.0) as i32;
            let y = (baseline + line_step * i as f32).max(0.0) as i32;
            draw_text_mut(img, fg, x, y, *scale, &self.label_font, text);
        }
    }

    fn line_width(&self, scale: PxScale, text: &str) -> f32 {
        text_size(scale, &self.label_font, text).0 as f32
    }

    fn ellipsize(&self, scale: PxScale, label: &str, budget: f32) -> String {
        let mut chars: Vec<char> = label.chars().collect();
        while chars.len() > 1 {
            chars.pop();
            let candidate: String = chars.iter().collect::<String>() + "…";
            if self.line_width(scale, &candidate) <= budget {
                return candidate;
            }
        }
        label.chars().take(1).collect()
    }
}

const LABEL_SCALE_MAX: f32 = 0.18;
const LABEL_SCALE_MIN: f32 = 0.10;
const LABEL_SCALE_STEP: f32 = 0.01;
const LABEL_WIDTH_BUDGET: f32 = 0.92;

/// Splits a label at the space closest to the middle, for two-line layout.
fn split_label(label: &str) -> Option<(String, String)> {
    let mid = label.chars().count() / 2;
    let breaks: Vec<usize> = label
        .char_indices()
        .filter(|(_, c)| *c == ' ')
        .map(|(i, _)| i)
        .collect();
    let best = breaks
        .into_iter()
        .min_by_key(|i| label[..*i].chars().count().abs_diff(mid))?;
    let head = label[..best].trim().to_string();
    let tail = label[best + 1..].trim().to_string();
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    Some((head, tail))
}

/// Reads a configured font, asking fontconfig for a stand-in when the path does
/// not exist. The defaults name Arch paths, which no other distribution has.
fn read_font(path: &str, fallback_family: &str) -> Result<Vec<u8>> {
    if let Ok(bytes) = fs::read(path) {
        return Ok(bytes);
    }
    let matched = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", fallback_family])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|found| !found.is_empty())
        .ok_or_else(|| anyhow::anyhow!("no font at {path} and fontconfig found no {fallback_family}"))?;
    eprintln!("streamdeck-ctl: {path} missing, using {matched}");
    Ok(fs::read(&matched)?)
}

fn parse_hex(s: &str) -> Result<Rgb<u8>> {
    let h = s.trim().trim_start_matches('#');
    // Checked before slicing: a six-byte value like "#aééb" has no char
    // boundary at index 2 and would panic the daemon on a config value the
    // panel and TUI both accept.
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("expected #RRGGBB, got {}", s);
    }
    let r = u8::from_str_radix(&h[0..2], 16)?;
    let g = u8::from_str_radix(&h[2..4], 16)?;
    let b = u8::from_str_radix(&h[4..6], 16)?;
    Ok(Rgb([r, g, b]))
}
