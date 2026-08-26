use crate::config::Config;
use crate::hypr::Monitor;
use crate::obs::Region;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    let screen = usable_from_monitor(mon);
    let usable = match obs_region.and_then(|r| screen.intersection(Rect {
        left: r.x,
        top: r.y,
        right: r.x.saturating_add(r.w),
        bottom: r.y.saturating_add(r.h),
    })) {
        Some(r) => r,
        None => screen,
    };

    if let Some(rect) = parse_rect(position)
        && !fullscreen
    {
        return keep_visible(rect, monitor_rect(mon));
    }

    if fullscreen {
        return Placement {
            x: usable.left,
            y: usable.top,
            w: usable.right - usable.left,
            h: usable.bottom - usable.top,
        };
    }

    let w = i32::try_from(cfg.size[0]).unwrap_or(i32::MAX).min(usable.width()).max(1);
    let h = i32::try_from(cfg.size[1]).unwrap_or(i32::MAX).min(usable.height()).max(1);
    let m = i32::try_from(cfg.margin).unwrap_or(i32::MAX);
    let (wanted_x, wanted_y) = match position {
        "top-left" => (usable.left.saturating_add(m), usable.top.saturating_add(m)),
        "top-right" => (
            usable.right.saturating_sub(w).saturating_sub(m),
            usable.top.saturating_add(m),
        ),
        "bottom-left" => (
            usable.left.saturating_add(m),
            usable.bottom.saturating_sub(h).saturating_sub(m),
        ),
        _ => (
            usable.right.saturating_sub(w).saturating_sub(m),
            usable.bottom.saturating_sub(h).saturating_sub(m),
        ),
    };
    let x = wanted_x.clamp(usable.left, usable.right - w);
    let y = wanted_y.clamp(usable.top, usable.bottom - h);
    Placement { x, y, w, h }
}

impl Rect {
    fn width(self) -> i32 {
        (self.right - self.left).max(1)
    }

    fn height(self) -> i32 {
        (self.bottom - self.top).max(1)
    }

    fn intersection(self, other: Rect) -> Option<Rect> {
        let out = Rect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (out.right > out.left && out.bottom > out.top).then_some(out)
    }
}

fn monitor_rect(mon: &Monitor) -> Rect {
    let scale = if mon.scale > 0.0 { mon.scale } else { 1.0 };
    Rect {
        left: mon.x,
        top: mon.y,
        right: mon.x.saturating_add((mon.width as f32 / scale).round() as i32),
        bottom: mon.y.saturating_add((mon.height as f32 / scale).round() as i32),
    }
}

fn keep_visible(rect: Placement, bounds: Rect) -> Placement {
    if rect.x >= bounds.left
        && rect.y >= bounds.top
        && rect.x.saturating_add(rect.w) <= bounds.right
        && rect.y.saturating_add(rect.h) <= bounds.bottom
    {
        return rect;
    }
    let w = rect.w.min(bounds.width()).max(1);
    let h = rect.h.min(bounds.height()).max(1);
    Placement {
        x: rect.x.clamp(bounds.left, bounds.right - w),
        y: rect.y.clamp(bounds.top, bounds.bottom - h),
        w,
        h,
    }
}

pub fn panel_rect(mon: &Monitor, w: i32, h: i32) -> Placement {
    let usable = usable_from_monitor(mon);
    let w = w.min(usable.width()).max(1);
    let h = h.min(usable.height()).max(1);
    Placement { x: usable.right - w, y: usable.top, w, h }
}

fn overlaps(a: &Placement, b: &Placement) -> bool {
    a.x < b.x.saturating_add(b.w)
        && b.x < a.x.saturating_add(a.w)
        && a.y < b.y.saturating_add(b.h)
        && b.y < a.y.saturating_add(a.h)
}

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

    let candidates = [
        Placement {
            x: blocker.x.saturating_sub(overlay.w).saturating_sub(margin),
            ..*overlay
        },
        Placement {
            x: blocker.x.saturating_add(blocker.w).saturating_add(margin),
            ..*overlay
        },
        Placement {
            y: blocker.y.saturating_add(blocker.h).saturating_add(margin),
            ..*overlay
        },
        Placement {
            y: blocker.y.saturating_sub(overlay.h).saturating_sub(margin),
            ..*overlay
        },
    ];
    candidates.into_iter().find(|p| {
        p.x >= usable.left
            && p.y >= usable.top
            && p.x.saturating_add(p.w) <= usable.right
            && p.y.saturating_add(p.h) <= usable.bottom
            && !overlaps(p, blocker)
    })
}

fn usable_from_monitor(mon: &Monitor) -> Rect {
    let screen = monitor_rect(mon);
    Rect {
        left: screen.left.saturating_add(mon.reserved[0]),
        top: screen.top.saturating_add(mon.reserved[1]),
        right: screen.right.saturating_sub(mon.reserved[2]).max(
            screen.left.saturating_add(mon.reserved[0]).saturating_add(1),
        ),
        bottom: screen.bottom.saturating_sub(mon.reserved[3]).max(
            screen.top.saturating_add(mon.reserved[1]).saturating_add(1),
        ),
    }
}

pub fn parse_rect(position: &str) -> Option<Placement> {
    let parts: Vec<i32> = position
        .strip_prefix("rect:")?
        .split(',')
        .map(|v| v.trim().parse().ok())
        .collect::<Option<Vec<i32>>>()?;
    let [x, y, w, h] = parts[..] else { return None };
    (w > 0 && h > 0 && x.checked_add(w).is_some() && y.checked_add(h).is_some())
        .then_some(Placement { x, y, w, h })
}

pub fn rect_to_position(p: &Placement) -> String {
    format!("rect:{},{},{},{}", p.x, p.y, p.w, p.h)
}

pub fn size_from_text(text: &str) -> Option<(i32, i32)> {
    let (w, h) = text.trim().split_once('x')?;
    let w: i32 = w.trim().parse().ok()?;
    let h: i32 = h.trim().parse().ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

pub fn rect_from_geometry(geometry: &str) -> Option<String> {
    let (origin, size) = geometry.trim().split_once(char::is_whitespace)?;
    let (x, y) = origin.split_once(',')?;
    let (w, h) = size.split_once('x')?;
    let x: i32 = x.trim().parse().ok()?;
    let y: i32 = y.trim().parse().ok()?;
    let w: i32 = w.trim().parse().ok()?;
    let h: i32 = h.trim().parse().ok()?;
    (w > 0 && h > 0 && x.checked_add(w).is_some() && y.checked_add(h).is_some())
        .then(|| format!("rect:{x},{y},{w},{h}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> Monitor {
        Monitor {
            id: 7,
            name: "DP-2".into(),
            x: 1920,
            y: -100,
            width: 3000,
            height: 1800,
            scale: 1.5,
            focused: false,
            reserved: [10, 40, 20, 30],
        }
    }

    #[test]
    fn intersects_obs_region_with_reserved_area() {
        let p = placement(
            &Config::default(),
            &monitor(),
            "bottom-right",
            true,
            Some(Region { x: 1900, y: -100, w: 2100, h: 1100 }),
        );
        assert_eq!(p, Placement { x: 1930, y: -60, w: 1970, h: 1060 });
    }

    #[test]
    fn ignores_an_obs_region_outside_the_monitor() {
        let p = placement(
            &Config::default(),
            &monitor(),
            "bottom-right",
            true,
            Some(Region { x: -4000, y: -4000, w: 100, h: 100 }),
        );
        assert_eq!(p, Placement { x: 1930, y: -60, w: 1970, h: 1130 });
    }

    #[test]
    fn brings_a_persisted_rectangle_back_after_a_display_change() {
        let p = placement(
            &Config::default(),
            &monitor(),
            "rect:-1200,600,600,400",
            false,
            None,
        );
        assert_eq!(p, Placement { x: 1920, y: 600, w: 600, h: 400 });
    }

    #[test]
    fn fits_an_oversized_corner_placement_on_screen() {
        let cfg = Config { size: [u32::MAX, u32::MAX], margin: u32::MAX, ..Config::default() };
        let p = placement(&cfg, &monitor(), "bottom-right", false, None);
        assert_eq!(p, Placement { x: 1930, y: -60, w: 1970, h: 1130 });
    }

    #[test]
    fn dodges_to_the_other_side_of_a_left_panel() {
        let overlay = Placement { x: 1930, y: 0, w: 320, h: 180 };
        let blocker = Placement { x: 1930, y: -60, w: 400, h: 600 };
        assert_eq!(
            dodge(&overlay, &blocker, &monitor(), 18),
            Some(Placement { x: 2348, ..overlay })
        );
    }
}
