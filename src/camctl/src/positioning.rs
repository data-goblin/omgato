use crate::config::Config;
use crate::hypr::Monitor;
use crate::obs::Region;

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

pub fn placement(
    cfg: &Config,
    mon: &Monitor,
    position: &str,
    fullscreen: bool,
    obs_region: Option<Region>,
) -> Placement {
    let usable = if let Some(r) = obs_region {
        Rect { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h }
    } else {
        usable_from_monitor(mon)
    };

    if let Some(rect) = parse_rect(position)
        && !fullscreen {
            return rect;
        }

    if fullscreen {
        return Placement {
            x: usable.left,
            y: usable.top,
            w: usable.right - usable.left,
            h: usable.bottom - usable.top,
        };
    }

    let w = cfg.size[0] as i32;
    let h = cfg.size[1] as i32;
    let m = cfg.margin as i32;
    let (x, y) = match position {
        "top-left"     => (usable.left + m,           usable.top + m),
        "top-right"    => (usable.right - w - m,      usable.top + m),
        "bottom-left"  => (usable.left + m,           usable.bottom - h - m),
        _              => (usable.right - w - m,      usable.bottom - h - m),
    };
    Placement { x, y, w, h }
}

/// A panel hanging from the bar at the right edge, given only its size. The
/// bar widget lives on the right, so that is where its panel appears.
pub fn panel_rect(mon: &Monitor, w: i32, h: i32) -> Placement {
    let usable = usable_from_monitor(mon);
    Placement { x: usable.right - w, y: usable.top, w, h }
}

fn overlaps(a: &Placement, b: &Placement) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// Slide the overlay clear of `blocker`, preferring to move left because a
/// panel occupies a full-height column. Falls back to moving down when there is
/// no room on the left. Returns None when it already misses the blocker, so the
/// caller can leave a position the user chose alone.
pub fn dodge(
    overlay: &Placement,
    blocker: &Placement,
    mon: &Monitor,
    margin: i32,
) -> Option<Placement> {
    if !overlaps(overlay, blocker) {
        return None;
    }
    let usable = usable_from_monitor(mon);

    let left = blocker.x - overlay.w - margin;
    if left >= usable.left {
        return Some(Placement { x: left, ..*overlay });
    }

    let below = blocker.y + blocker.h + margin;
    if below + overlay.h <= usable.bottom {
        return Some(Placement { y: below, ..*overlay });
    }

    Some(Placement { x: usable.left + margin, ..*overlay })
}

fn usable_from_monitor(mon: &Monitor) -> Rect {
    let scale = if mon.scale > 0.0 { mon.scale } else { 1.0 };
    let mw = (mon.width as f32 / scale) as i32;
    let mh = (mon.height as f32 / scale) as i32;
    Rect {
        left:   mon.x + mon.reserved[0],
        top:    mon.y + mon.reserved[1],
        right:  mon.x + mw - mon.reserved[2],
        bottom: mon.y + mh - mon.reserved[3],
    }
}

/// Reads a pinned rectangle back out of the persisted position, which holds
/// "rect:X,Y,W,H" when the overlay was placed by hand rather than by corner.
pub fn parse_rect(position: &str) -> Option<Placement> {
    let parts: Vec<i32> = position
        .strip_prefix("rect:")?
        .split(',')
        .map(|v| v.trim().parse().ok())
        .collect::<Option<Vec<i32>>>()?;
    let [x, y, w, h] = parts[..] else { return None };
    (w > 0 && h > 0).then_some(Placement { x, y, w, h })
}

/// Renders a placement back into the persisted "rect:X,Y,W,H" form.
pub fn rect_to_position(p: &Placement) -> String {
    format!("rect:{},{},{},{}", p.x, p.y, p.w, p.h)
}

/// Reads a bare "WxH" size, which is how a panel reports itself.
pub fn size_from_text(text: &str) -> Option<(i32, i32)> {
    let (w, h) = text.trim().split_once('x')?;
    let w: i32 = w.trim().parse().ok()?;
    let h: i32 = h.trim().parse().ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

/// Normalises slurp's "X,Y WxH" into the persisted "rect:X,Y,W,H" form.
pub fn rect_from_geometry(geometry: &str) -> Option<String> {
    let (origin, size) = geometry.trim().split_once(char::is_whitespace)?;
    let (x, y) = origin.split_once(',')?;
    let (w, h) = size.split_once('x')?;
    let x: i32 = x.trim().parse().ok()?;
    let y: i32 = y.trim().parse().ok()?;
    let w: i32 = w.trim().parse().ok()?;
    let h: i32 = h.trim().parse().ok()?;
    (w > 0 && h > 0).then(|| format!("rect:{x},{y},{w},{h}"))
}
