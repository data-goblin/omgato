use std::process::{Command, Stdio};

pub fn run(cmd: &[&str]) -> String {
    let Some((bin, args)) = cmd.split_first() else {
        return String::new();
    };
    match Command::new(bin).args(args).output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

/// Starts a command and lets it go. Used where the child outlives the call and
/// capturing its output would block on a pipe the child keeps open.
pub fn spawn_detached(cmd: &[String]) -> Option<u32> {
    let (bin, args) = cmd.split_first()?;
    Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .map(|child| child.id())
}

/// Runs a command and reports whether it succeeded, for callers that must not
/// treat a failure as an empty result.
pub fn succeeded(cmd: &[&str]) -> bool {
    let Some((bin, args)) = cmd.split_first() else {
        return false;
    };
    Command::new(bin)
        .args(args)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn run_owned(cmd: &[String]) -> String {
    let borrowed: Vec<&str> = cmd.iter().map(String::as_str).collect();
    run(&borrowed)
}

/// Runs every command at once and returns their stdout in input order.
pub fn run_all(cmds: &[Vec<String>]) -> Vec<String> {
    if cmds.len() < 2 {
        return cmds.iter().map(|c| run_owned(c)).collect();
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = cmds.iter().map(|c| scope.spawn(move || run_owned(c))).collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    })
}
