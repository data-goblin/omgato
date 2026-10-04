use ab_glyph::{Font, FontVec, GlyphId, PxScale, ScaleFont, point};

pub struct Page {
    lines: Vec<(usize, String)>,
    scale: PxScale,
    ascent: f32,
    margin: usize,
    pub line: usize,
    pub lead: usize,
    pub height: usize,
    width: usize,
    screen: usize,
}

pub fn load_font() -> Result<FontVec, String> {
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}", "sans-serif:bold"]).output()
        .map_err(|e| format!("fc-match: {e}"))?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let bytes = std::fs::read(&path).map_err(|e| format!("font {path:?}: {e}"))?;
    FontVec::try_from_vec(bytes).map_err(|e| format!("font {path}: {e}"))
}

pub fn layout(font: &FontVec, text: &str, px: f32, spacing: f32, width: usize, height: usize) -> Page {
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    let line = (sf.height() * spacing).ceil() as usize;
    let margin = width / 16;
    let room = width.saturating_sub(2 * margin) as f32;
    let space = sf.h_advance(font.glyph_id(' '));
    let measure = |w: &str| w.chars().map(|c| sf.h_advance(font.glyph_id(c))).sum::<f32>();
    let lead = height / 3;
    let mut lines = Vec::new();
    let mut row = 0;
    for para in text.lines() {
        let para = para.trim_start_matches('#').trim();
        let mut cur = String::new();
        let mut cur_w = 0.0;
        for word in para.split_whitespace() {
            let ww = measure(word);
            if !cur.is_empty() && cur_w + space + ww > room {
                lines.push((lead + row * line, std::mem::take(&mut cur)));
                row += 1;
                cur_w = 0.0;
            }
            if !cur.is_empty() {
                cur.push(' ');
                cur_w += space;
            }
            cur.push_str(word);
            cur_w += ww;
        }
        if !cur.is_empty() {
            lines.push((lead + row * line, cur));
        }
        row += 1;
    }
    Page { lines, scale, ascent: sf.ascent(), margin, line, lead, height: lead + row * line + (height - lead), width, screen: height }
}

pub fn draw(font: &FontVec, page: &Page, offset: usize, mirror: bool, dst: &mut [u8], pitch: usize) {
    let (width, height) = (page.width, page.screen);
    dst.fill(0);
    let mut put = |x: i32, y: i32, v: u8| {
        if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
            return;
        }
        let x = if mirror { width - 1 - x as usize } else { x as usize };
        let i = y as usize * pitch + x * 4;
        let v = v.max(dst[i]);
        dst[i..i + 4].copy_from_slice(&[v, v, v, 0xff]);
    };
    let sf = font.as_scaled(page.scale);
    for (top, text) in &page.lines {
        if top + page.line < offset || *top > offset + height {
            continue;
        }
        let baseline = *top as f32 - offset as f32 + page.ascent;
        let mut x = page.margin as f32;
        let mut prev: Option<GlyphId> = None;
        for c in text.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                x += sf.kern(p, id);
            }
            if let Some(o) = font.outline_glyph(id.with_scale_and_position(page.scale, point(x, baseline))) {
                let b = o.px_bounds();
                o.draw(|gx, gy, v| put(b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, (v * 255.0) as u8));
            }
            x += sf.h_advance(id);
            prev = Some(id);
        }
    }
    let mid = (page.lead + page.line / 2) as i32;
    for dy in -10..=10i32 {
        for dx in 0..(10 - dy.abs()) {
            put(8 + dx, mid + dy, 0x70);
        }
    }
}
