use crate::config::{FieldMeta, Mqtt};
use serde_json::{Value, json};

fn sanitize(field: &str) -> String {
    field.replace('.', "_")
}

pub fn unique_id(box_id: u64, field: &str) -> String {
    format!("hetzner_storage_box_{box_id}_{}", sanitize(field))
}

pub fn state_topic(mqtt: &Mqtt, box_id: u64, field: &str) -> String {
    format!("{}/{box_id}/{}/state", mqtt.base_topic, sanitize(field))
}

pub fn discovery_topic(mqtt: &Mqtt, box_id: u64, field: &str) -> String {
    format!(
        "{}/sensor/hetzner_storage_box_{box_id}/{}/config",
        mqtt.discovery_prefix,
        sanitize(field)
    )
}

/// Builds a Home Assistant MQTT discovery config payload for one field of one storage
/// box. All sensors for a box share a `device.identifiers` value so they group under
/// one HA device. `meta` is optional per-field metadata (friendly name, unit,
/// device class) — when absent, discovery still works, just with less nice defaults.
pub fn discovery_payload(
    mqtt: &Mqtt,
    box_id: u64,
    box_name: &str,
    field: &str,
    meta: Option<&FieldMeta>,
) -> Value {
    let name = meta
        .and_then(|m| m.friendly_name.clone())
        .unwrap_or_else(|| field.to_string());

    let mut payload = json!({
        "name": name,
        "state_topic": state_topic(mqtt, box_id, field),
        "unique_id": unique_id(box_id, field),
        "device": {
            "identifiers": [format!("hetzner_storage_box_{box_id}")],
            "name": box_name,
            "manufacturer": "Hetzner",
            "model": "Storage Box",
        },
    });

    if let Some(meta) = meta {
        if let Some(unit) = &meta.unit_of_measurement {
            payload["unit_of_measurement"] = json!(unit);
        }
        if let Some(device_class) = &meta.device_class {
            payload["device_class"] = json!(device_class);
        }
    }

    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mqtt_config() -> Mqtt {
        Mqtt {
            host: "mqtt.example.local".to_string(),
            port: 1883,
            username: "mqtt-user".to_string(),
            password: "mqtt-pass".to_string(),
            client_id: "hetzner-storage-box-to-mqtt".to_string(),
            base_topic: "hetzner_storage_box".to_string(),
            discovery_prefix: "homeassistant".to_string(),
            tls: false,
        }
    }

    #[test]
    fn state_topic_uses_base_topic_box_id_and_sanitized_field() {
        let mqtt = mqtt_config();
        assert_eq!(
            state_topic(&mqtt, 42, "stats.size"),
            "hetzner_storage_box/42/stats_size/state"
        );
    }

    #[test]
    fn discovery_topic_uses_prefix_and_sanitized_field() {
        let mqtt = mqtt_config();
        assert_eq!(
            discovery_topic(&mqtt, 42, "stats.size"),
            "homeassistant/sensor/hetzner_storage_box_42/stats_size/config"
        );
    }

    #[test]
    fn payload_without_meta_omits_optional_keys() {
        let mqtt = mqtt_config();
        let payload = discovery_payload(&mqtt, 42, "my-resource", "stats.size", None);

        assert_eq!(
            payload,
            json!({
                "name": "stats.size",
                "state_topic": "hetzner_storage_box/42/stats_size/state",
                "unique_id": "hetzner_storage_box_42_stats_size",
                "device": {
                    "identifiers": ["hetzner_storage_box_42"],
                    "name": "my-resource",
                    "manufacturer": "Hetzner",
                    "model": "Storage Box",
                }
            })
        );
    }

    #[test]
    fn payload_with_meta_includes_optional_keys() {
        let mqtt = mqtt_config();
        let meta = FieldMeta {
            friendly_name: Some("Used Size".to_string()),
            unit_of_measurement: Some("B".to_string()),
            device_class: Some("data_size".to_string()),
        };
        let payload = discovery_payload(&mqtt, 42, "my-resource", "stats.size", Some(&meta));

        assert_eq!(payload["name"], json!("Used Size"));
        assert_eq!(payload["unit_of_measurement"], json!("B"));
        assert_eq!(payload["device_class"], json!("data_size"));
    }
}
