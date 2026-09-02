use crate::config::{Button, Config};
use crate::device;
use crate::render::Renderer;
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

const FALLBACK_SIZE: u32 = 72;
const FALLBACK_KEYS: u8 = 15;
const FALLBACK_COLS: u8 = 5;

pub fn run(
    cfg: &Config,
    out: &Path,
    page: Option<String>,
    size: Option<u32>,
    keys: Option<u8>,
    radius: Option<f32>,
) -> Result<()> {
    let out = absolute(out)?;
    fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;
    verify_trusted_directory(&out)?;
    let device = device::deck::open_first().ok();
    let size = size
        .or_else(|| device.as_ref().map(|d| d.kind.key_image_format().size.0 as u32))
        .unwrap_or(FALLBACK_SIZE);
    let key_count = keys
        .or_else(|| device.as_ref().map(|d| d.kind.key_count()))
        .unwrap_or(FALLBACK_KEYS);

    let cols = device
        .as_ref()
        .map(|d| d.kind.column_count())
        .unwrap_or(FALLBACK_COLS);
    let renderer = Renderer::new(&cfg.deck, size)?;
    let pages: Vec<String> = match page {
        Some(name) => vec![name],
        None => cfg.deck.pages.keys().cloned().collect(),
    };

    for name in pages {
        validate_page_name(&name)?;
        let Some(page) = cfg.deck.pages.get(&name) else {
            anyhow::bail!("page not found: {name}");
        };
        let dir = out.join(&name);
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        verify_trusted_directory(&dir)?;
        let by_index: HashMap<u8, &Button> = page.buttons.iter().map(|b| (b.index, b)).collect();
        for index in 0..key_count {
            let image = match by_index.get(&index) {
                Some(btn) => renderer.render_button(btn, page.bg.as_deref())?,
                None => match cfg.deck.synthetic_button(&name, index, key_count, cols) {
                    Some(synth) => renderer.render_button(&synth, None)?,
                    None => renderer.blank(),
                },
            };
            let path = dir.join(format!("{index}.png"));
            let image = match radius {
                Some(radius) if radius > 0.0 => round_corners(&image, radius),
                _ => image,
            };
            write_png_atomic(&image, &path)?;
        }
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir().context("resolve export directory")?.join(path))
    }
}

fn validate_page_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || name.contains('\\')
    {
        anyhow::bail!("page name cannot be used as an export directory: {name}");
    }
    Ok(())
}

fn verify_trusted_directory(path: &Path) -> Result<()> {
    let own_uid = users_own_uid();
    let root_uid = fs::symlink_metadata("/").context("stat /")?.uid();
    let mut current = PathBuf::from("/");
    for component in path.components().skip(1) {
        current.push(component.as_os_str());
        let meta = fs::symlink_metadata(&current)
            .with_context(|| format!("stat {}", current.display()))?;
        if !meta.is_dir() {
            anyhow::bail!("{} is not a real directory", current.display());
        }
        if meta.uid() != own_uid && meta.uid() != root_uid {
            anyhow::bail!("{} is owned by untrusted uid {}", current.display(), meta.uid());
        }
        let mode = meta.permissions().mode() & 0o7777;
        if mode & 0o022 != 0 && mode & 0o1000 == 0 {
            anyhow::bail!(
                "{} is writable by other users without the sticky bit (mode {mode:o})",
                current.display()
            );
        }
    }
    Ok(())
}

fn write_png_atomic(image: &DynamicImage, path: &Path) -> Result<()> {
    let parent = path.parent().context("PNG path has no parent")?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("key.png"),
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)
        .with_context(|| format!("create {}", tmp.display()))?;
    if let Err(e) = image.write_to(&mut file, ImageFormat::Png) {
        drop(file);
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("encode {}", path.display()));
    }
    file.sync_all().with_context(|| format!("sync {}", tmp.display()))?;
    drop(file);
    fs::rename(&tmp, path).with_context(|| format!("publish {}", path.display()))
}

fn users_own_uid() -> u32 {
    unsafe { getuid() }
}

unsafe extern "C" {
    fn getuid() -> u32;
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, symlink};

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("streamdeck-export-{}-{name}", std::process::id()))
    }

    #[test]
    fn rejects_page_names_that_escape_the_export_root() {
        assert!(validate_page_name("main").is_ok());
        assert!(validate_page_name("../../escape").is_err());
        assert!(validate_page_name("/tmp/escape").is_err());
    }

    #[test]
    fn publishing_a_png_replaces_a_symlink_without_following_it() {
        let dir = scratch("symlink");
        let _ = fs::remove_dir_all(&dir);
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let outside = dir.join("outside");
        fs::write(&outside, b"untouched").unwrap();
        let path = dir.join("0.png");
        symlink(&outside, &path).unwrap();

        write_png_atomic(&DynamicImage::new_rgba8(1, 1), &path).unwrap();

        assert_eq!(fs::read(&outside).unwrap(), b"untouched");
        assert!(fs::symlink_metadata(&path).unwrap().file_type().is_file());
        fs::remove_dir_all(&dir).unwrap();
    }
}
