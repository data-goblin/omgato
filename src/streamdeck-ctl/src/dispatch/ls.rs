use crate::device;
use anyhow::Result;

pub fn run(json: bool) -> Result<()> {
    let all = device::describe_all()?;
    if json {
        println!("{}", serde_json::to_string(&all)?);
        return Ok(());
    }
    if all.is_empty() {
        println!("no Stream Deck devices connected");
        return Ok(());
    }
    for d in all {
        println!(
            "{}  serial={}  keys={}  grid={}x{}",
            d.kind, d.serial, d.keys, d.cols, d.rows
        );
    }
    Ok(())
}
