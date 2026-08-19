pub mod config;
pub mod discovery;
pub mod fields;
pub mod hetzner;
pub mod mqtt;
pub mod retry;

use anyhow::{Context, Result, bail};
use config::Config;
use hetzner::HetznerClient;

/// Fetches every configured, publish-enabled storage box, extracts its selected
/// fields, and publishes state + Home Assistant discovery messages for all of them
/// in one MQTT session.
///
/// A single box's fetch failure doesn't abort the run — the other boxes still get
/// published — but the run still exits with an error at the end so cron/systemd
/// alerting notices.
pub async fn run(config: &Config) -> Result<()> {
    let client = HetznerClient::new(
        config.hetzner.api_base_url.clone(),
        config.hetzner.api_token.clone(),
    );
    let retry_enabled = config.general.retry_enabled;

    let mut messages = Vec::new();
    let mut any_box_failed = false;

    for sb in &config.storage_box {
        if !sb.publish {
            continue;
        }

        let fetch_result =
            retry::with_retry(retry_enabled, || client.fetch_storage_box(sb.id)).await;

        let box_data = match fetch_result {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(box_id = sb.id, error = %e, "Storage box fetch failed");
                any_box_failed = true;
                continue;
            }
        };

        tracing::info!(box_id = box_data.id, box_data = ?box_data, "Fetched storage box data");

        let device_name = sb.alias.as_deref().unwrap_or(&box_data.name);

        for field in &sb.fields {
            let Some(value) = fields::extract_field(&box_data, field) else {
                tracing::warn!(
                    box_id = sb.id,
                    field = %field,
                    "Field not currently available, skipping"
                );
                continue;
            };

            let meta = sb.field_meta.get(field);
            messages.push(mqtt::Message {
                topic: discovery::state_topic(&config.mqtt, sb.id, field),
                payload: value.to_string(),
                retain: true,
            });
            messages.push(mqtt::Message {
                topic: discovery::discovery_topic(&config.mqtt, sb.id, field),
                payload: discovery::discovery_payload(
                    &config.mqtt,
                    sb.id,
                    device_name,
                    field,
                    meta,
                )
                .to_string(),
                retain: true,
            });
        }
    }

    if !messages.is_empty() {
        retry::with_retry(retry_enabled, || mqtt::publish_all(&config.mqtt, &messages))
            .await
            .context("publishing to MQTT")?;
    }

    if any_box_failed {
        bail!("one or more storage boxes failed to fetch — see errors above");
    }
    Ok(())
}
