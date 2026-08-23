use anyhow::Result;

const UDEV_RULE: &str = include_str!("../../udev/70-streamdeck-ctl.rules");

pub fn install() -> Result<()> {
    let target = "/etc/udev/rules.d/70-streamdeck-ctl.rules";
    println!("--- {} ---", target);
    println!("{}", UDEV_RULE);
    println!("Install with:");
    println!("  sudo tee {target} <<'EOF'");
    print!("{UDEV_RULE}");
    println!("EOF");
    println!("  sudo udevadm control --reload-rules && sudo udevadm trigger");
    Ok(())
}
