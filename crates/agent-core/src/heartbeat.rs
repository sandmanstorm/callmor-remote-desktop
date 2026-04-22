use serde::Serialize;
use tracing::{debug, warn};

#[derive(Serialize)]
struct HeartbeatBody<'a> {
    machine_id: &'a str,
    hostname: &'a str,
    os: &'a str,
}

/// Run a heartbeat loop that POSTs to {api_url}/agent/heartbeat every interval.
pub async fn run(
    api_url: String,
    agent_token: String,
    machine_id: String,
    hostname: String,
    os: &'static str,
    interval_secs: u64,
) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client");

    let endpoint = format!("{api_url}/agent/heartbeat");
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;
        let result = client
            .post(&endpoint)
            .header("X-Agent-Token", &agent_token)
            .json(&HeartbeatBody {
                machine_id: &machine_id,
                hostname: &hostname,
                os,
            })
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                debug!("Heartbeat OK");
            }
            Ok(resp) => warn!("Heartbeat HTTP {}", resp.status()),
            Err(e) => warn!("Heartbeat error: {e}"),
        }
    }
}
