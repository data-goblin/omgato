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
pub fn spawn_detached(cmd: &[String]) {
    let Some((bin, args)) = cmd.split_first() else {
        return;
    };
    let _ = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
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
