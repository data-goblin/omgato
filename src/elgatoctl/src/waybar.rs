use crate::config::Cache;
use crate::light;

pub fn emit(cache: &Cache) {
    if cache.lights.is_empty() {
        println!(
            "{}",
            serde_json::json!({
                "text": "",
                "alt": "off",
                "class": "off",
                "tooltip": "no lights cached - run: elgatoctl discover"
            })
        );
        return;
    }

    let mut any_on = false;
    let mut tooltip_lines: Vec<String> = Vec::new();
    let mut unreachable = 0;

    for (l, state) in cache.lights.iter().zip(light::probe(&cache.lights)) {
        match state {
            Ok(s) => {
                if s.on == 1 {
                    any_on = true;
                }
                let kelvin = light::mired_to_kelvin(s.temperature);
                tooltip_lines.push(format!(
                    "{}: {} {}% {}K",
                    l.name,
                    if s.on == 1 { "on" } else { "off" },
                    s.brightness,
                    kelvin
                ));
            }
            Err(_) => {
                unreachable += 1;
                tooltip_lines.push(format!("{}: unreachable", l.name));
            }
        }
    }

    let class = if unreachable == cache.lights.len() {
        "unreachable"
    } else if any_on {
        "on"
    } else {
        "off"
    };
    let alt = class;

    println!(
        "{}",
        serde_json::json!({
            "text": "",
            "alt": alt,
            "class": class,
            "tooltip": tooltip_lines.join("\n")
        })
    );
}
