use crate::config::Light;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightState {
    #[serde(default)]
    pub on: u8,
    #[serde(default)]
    pub brightness: u8,
    #[serde(default)]
    pub temperature: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LightsResponse {
    #[serde(rename = "numberOfLights", default)]
    pub number_of_lights: u8,
    pub lights: Vec<LightState>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct LightPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<u16>,
}

impl LightPatch {
    pub fn is_empty(&self) -> bool {
        self.on.is_none() && self.brightness.is_none() && self.temperature.is_none()
    }
}

const TIMEOUT_MS: u64 = 1500;
const CONNECT_TIMEOUT_MS: u64 = 600;

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_millis(CONNECT_TIMEOUT_MS))
            .timeout(Duration::from_millis(TIMEOUT_MS))
            .max_idle_connections_per_host(4)
            .build()
    })
}

pub fn get_state(light: &Light) -> Result<LightState, String> {
    let resp: LightsResponse = agent()
        .get(&light.url("/elgato/lights"))
        .call()
        .map_err(|e| format!("{}: {e}", light.name))?
        .into_json()
        .map_err(|e| format!("{}: parse: {e}", light.name))?;
    resp.lights
        .into_iter()
        .next()
        .ok_or_else(|| format!("{}: empty lights array", light.name))
}

pub fn apply(light: &Light, patch: &LightPatch) -> Result<LightState, String> {
    let body = serde_json::json!({ "lights": [patch] });
    let resp: LightsResponse = agent()
        .put(&light.url("/elgato/lights"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("{}: {e}", light.name))?
        .into_json()
        .map_err(|e| format!("{}: parse: {e}", light.name))?;
    resp.lights
        .into_iter()
        .next()
        .ok_or_else(|| format!("{}: empty lights array", light.name))
}

/// Runs `f` against every light at once and returns the results in input order.
pub fn each<T, F>(lights: &[Light], f: F) -> Vec<T>
where
    F: Fn(&Light) -> T + Sync,
    T: Send,
{
    let f = &f;
    if lights.len() < 2 {
        return lights.iter().map(f).collect();
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = lights.iter().map(|l| scope.spawn(move || f(l))).collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| std::process::abort()))
            .collect()
    })
}

pub fn probe(lights: &[Light]) -> Vec<Result<LightState, String>> {
    each(lights, get_state)
}

pub fn kelvin_to_mired(k: u32) -> u16 {
    let m = 1_000_000 / k.max(1);
    m.clamp(143, 344) as u16
}

pub fn mired_to_kelvin(m: u16) -> u32 {
    if m == 0 {
        0
    } else {
        1_000_000 / m as u32
    }
}
