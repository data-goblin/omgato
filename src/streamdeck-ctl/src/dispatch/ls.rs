use crate::device;
use anyhow::Result;

pub fn run() -> Result<()> {
    let all = device::list_all()?;
    if all.is_empty() {
        println!("no Stream Deck devices connected");
        return Ok(());
    }
    for (kind, serial) in all {
        println!("{:?}  serial={}", kind, serial);
    }
    Ok(())
}
