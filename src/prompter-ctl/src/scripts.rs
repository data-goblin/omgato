use std::fs;
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("prompter-ctl").join("scripts")
}

fn valid(name: &str) -> Result<&str, String> {
    let n = name.trim();
    let ok = !n.is_empty()
        && n.chars().count() <= 64
        && n.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'));
    if ok { Ok(n) } else { Err(format!("script name {name:?}: use up to 64 letters, digits, spaces, - or _")) }
}

pub fn path(name: &str) -> Result<PathBuf, String> {
    Ok(dir().join(format!("{}.md", valid(name)?)))
}

pub fn list() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir()).into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".md").map(str::to_owned))
        .filter(|n| valid(n).is_ok())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

pub fn read(name: &str) -> Result<String, String> {
    let p = path(name)?;
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

pub fn write(name: &str, text: &str) -> Result<(), String> {
    let p = path(name)?;
    fs::create_dir_all(dir()).map_err(|e| format!("create {}: {e}", dir().display()))?;
    let tmp = p.with_extension("md.tmp");
    fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &p).map_err(|e| format!("save {}: {e}", p.display()))
}

pub fn remove(name: &str) -> Result<(), String> {
    let p = path(name)?;
    fs::remove_file(&p).map_err(|e| format!("remove {}: {e}", p.display()))
}
