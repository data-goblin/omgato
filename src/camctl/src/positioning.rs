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

    if let Some(rect) = parse_rect(position) {
        if !fullscreen {
            return rect;
        }
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

fn usable_from_monitor(mon: &Monitor) -> Rect {
    let scale = if mon.scale > 0.0 { mon.scale } else { 1.0 };
    let mw = (mon.width as f32 / scale) as i32;
    let mh = (mon.height as f32 / scale) as i32;
    // reserved = [left, top, right, bottom] from Hyprland layer shell.
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
