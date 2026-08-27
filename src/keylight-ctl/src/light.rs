use crate::config::Light;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
const ATTEMPTS: u8 = 3;
const RETRY_PAUSE_MS: u64 = 80;
const RETRY_BUDGET_MS: u64 = 500;
const MAX_HTTP_BODY_BYTES: usize = 16 * 1024;
const MAX_LIGHT_STATES: usize = 8;
const MAX_CONCURRENT_REQUESTS: usize = 4;

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

fn retrying<T>(mut attempt: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let started = Instant::now();
    let budget = Duration::from_millis(RETRY_BUDGET_MS);
    let mut result = attempt();
    for _ in 1..ATTEMPTS {
        if result.is_ok() || started.elapsed() >= budget {
            break;
        }
        std::thread::sleep(Duration::from_millis(RETRY_PAUSE_MS));
        result = attempt();
    }
    result
}

fn first(resp: LightsResponse, name: &str) -> Result<LightState, String> {
    if resp.lights.len() > MAX_LIGHT_STATES {
        return Err(format!("{name}: too many light states in response"));
    }
    resp.lights
        .into_iter()
        .next()
        .ok_or_else(|| format!("{name}: empty lights array"))
}

pub fn get_state(light: &Light) -> Result<LightState, String> {
    retrying(|| {
        let response = agent()
            .get(&light.url("/elgato/lights"))
            .call()
            .map_err(|e| format!("{}: {e}", light.name))?;
        let resp: LightsResponse = response_json(response, &light.name)?;
        first(resp, &light.name)
    })
}

pub fn apply(light: &Light, patch: &LightPatch) -> Result<LightState, String> {
    let body = serde_json::json!({ "lights": [patch] }).to_string();
    retrying(|| {
        let response = agent()
            .put(&light.url("/elgato/lights"))
            .set("Content-Type", "application/json")
            .send_string(&body)
            .map_err(|e| format!("{}: {e}", light.name))?;
        let resp: LightsResponse = response_json(response, &light.name)?;
        first(resp, &light.name)
    })
}

pub(crate) fn response_json<T: DeserializeOwned>(
    response: ureq::Response,
    context: &str,
) -> Result<T, String> {
    if response
        .header("Content-Length")
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_HTTP_BODY_BYTES)
    {
        return Err(format!("{context}: response body exceeds {MAX_HTTP_BODY_BYTES} bytes"));
    }
    json_from_reader(response.into_reader(), context)
}

fn json_from_reader<T: DeserializeOwned>(reader: impl Read, context: &str) -> Result<T, String> {
    let mut body = Vec::with_capacity(MAX_HTTP_BODY_BYTES.min(4096));
    reader
        .take((MAX_HTTP_BODY_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|e| format!("{context}: read: {e}"))?;
    if body.len() > MAX_HTTP_BODY_BYTES {
        return Err(format!("{context}: response body exceeds {MAX_HTTP_BODY_BYTES} bytes"));
    }
    serde_json::from_slice(&body).map_err(|e| format!("{context}: parse: {e}"))
}

pub fn each<T, F>(lights: &[Light], f: F) -> Vec<T>
where
    F: Fn(&Light) -> T + Sync,
    T: Send,
{
    let f = &f;
    if lights.len() < 2 {
        return lights.iter().map(f).collect();
    }
    let workers = lights.len().min(MAX_CONCURRENT_REQUESTS);
    let next = AtomicUsize::new(0);
    let slots = Mutex::new((0..lights.len()).map(|_| None).collect::<Vec<Option<T>>>());
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= lights.len() {
                        break;
                    }
                    let result = f(&lights[index]);
                    slots.lock().unwrap_or_else(|_| std::process::abort())[index] = Some(result);
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap_or_else(|_| std::process::abort());
        }
    });
    slots
        .into_inner()
        .unwrap_or_else(|_| std::process::abort())
        .into_iter()
        .map(|result| result.unwrap_or_else(|| std::process::abort()))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_light(index: usize) -> Light {
        Light {
            name: index.to_string(),
            ip: format!("192.0.2.{}", index + 1),
            port: 9123,
            mac: String::new(),
        }
    }

    #[test]
    fn rejects_oversized_http_json() {
        let body = vec![b' '; MAX_HTTP_BODY_BYTES + 1];
        let result = json_from_reader::<serde_json::Value>(Cursor::new(body), "test");
        assert!(result.unwrap_err().contains("exceeds"));
    }

    #[test]
    fn accepts_bounded_http_json() {
        let value: serde_json::Value =
            json_from_reader(Cursor::new(br#"{"ok":true}"#), "test").unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn caps_parallel_requests_and_preserves_order() {
        let lights: Vec<_> = (0..MAX_CONCURRENT_REQUESTS * 3).map(test_light).collect();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let threads = Mutex::new(HashSet::new());
        let results = each(&lights, |light| {
            threads.lock().unwrap().insert(std::thread::current().id());
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(5));
            active.fetch_sub(1, Ordering::SeqCst);
            light.name.clone()
        });

        assert!(peak.load(Ordering::SeqCst) <= MAX_CONCURRENT_REQUESTS);
        assert!(threads.lock().unwrap().len() <= MAX_CONCURRENT_REQUESTS);
        assert_eq!(results, lights.iter().map(|light| light.name.clone()).collect::<Vec<_>>());
    }
}
