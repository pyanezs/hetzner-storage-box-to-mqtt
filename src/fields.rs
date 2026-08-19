use crate::hetzner::StorageBox;
use serde_json::{Value, json};

/// Field paths selectable via a storage box's `fields`/`field_meta` config.
/// A curated, practical subset of `StorageBox` — not everything the struct models
/// is exposed here (e.g. pricing/deprecation/snapshot-plan details), since those
/// aren't useful for a home dashboard. Extending this list and the match in
/// `extract_field` below is how more fields get exposed later.
pub const KNOWN_FIELDS: &[&str] = &[
    "id",
    "name",
    "status",
    "username",
    "server",
    "system",
    "created",
    "stats.size",
    "stats.size_data",
    "stats.size_snapshots",
    "storage_box_type.name",
    "storage_box_type.description",
    "storage_box_type.size",
    "location.name",
    "location.city",
    "location.country",
    "location.network_zone",
    "access_settings.reachable_externally",
    "access_settings.samba_enabled",
    "access_settings.ssh_enabled",
    "access_settings.webdav_enabled",
    "access_settings.zfs_enabled",
    "protection.delete",
    "labels",
    "last_updated",
];

const BYTES_PER_MIB: f64 = 1_048_576.0;

/// Converts a byte count to MiB, rounded to 2 decimal places — the Hetzner API
/// reports storage sizes in bytes, but a home dashboard is more readable in MiB.
fn bytes_to_mib(bytes: u64) -> f64 {
    ((bytes as f64 / BYTES_PER_MIB) * 100.0).round() / 100.0
}

/// Extracts a known field's value from a storage box.
/// Returns `None` both for an unknown key and for a field whose value is currently
/// absent (e.g. `username` while the box is still `initializing`) — callers treat
/// both cases the same way: skip publishing it this run.
///
/// `last_updated` isn't part of `StorageBox` — it's an RFC3339 timestamp the caller
/// captures once per run, right before publishing.
pub fn extract_field(b: &StorageBox, key: &str, last_updated: &str) -> Option<Value> {
    match key {
        "last_updated" => Some(json!(last_updated)),
        "id" => Some(json!(b.id)),
        "name" => Some(json!(b.name)),
        "status" => serde_json::to_value(&b.status).ok(),
        "username" => b.username.clone().map(Value::from),
        "server" => b.server.clone().map(Value::from),
        "system" => b.system.clone().map(Value::from),
        "created" => Some(json!(b.created)),
        "stats.size" => Some(json!(bytes_to_mib(b.stats.size))),
        "stats.size_data" => Some(json!(bytes_to_mib(b.stats.size_data))),
        "stats.size_snapshots" => Some(json!(bytes_to_mib(b.stats.size_snapshots))),
        "storage_box_type.name" => Some(json!(b.storage_box_type.name)),
        "storage_box_type.description" => Some(json!(b.storage_box_type.description)),
        "storage_box_type.size" => Some(json!(b.storage_box_type.size)),
        "location.name" => Some(json!(b.location.name)),
        "location.city" => Some(json!(b.location.city)),
        "location.country" => Some(json!(b.location.country)),
        "location.network_zone" => Some(json!(b.location.network_zone)),
        "access_settings.reachable_externally" => {
            Some(json!(b.access_settings.reachable_externally))
        }
        "access_settings.samba_enabled" => Some(json!(b.access_settings.samba_enabled)),
        "access_settings.ssh_enabled" => Some(json!(b.access_settings.ssh_enabled)),
        "access_settings.webdav_enabled" => Some(json!(b.access_settings.webdav_enabled)),
        "access_settings.zfs_enabled" => Some(json!(b.access_settings.zfs_enabled)),
        "protection.delete" => Some(json!(b.protection.delete)),
        "labels" => Some(json!(b.labels)),
        _ => None,
    }
}

/// Converts an extracted field's JSON value into the raw string published as its MQTT
/// state payload. JSON strings are unwrapped rather than serialized with surrounding
/// quotes — Home Assistant's typed device classes (e.g. `timestamp`) parse the state
/// payload directly and treat a quoted string as invalid, showing the entity as
/// "Unknown" instead of the real value.
pub fn to_payload(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hetzner::{
        AccessSettings, Location, Protection, Stats, StorageBoxStatus, StorageBoxType,
    };
    use std::collections::HashMap;

    fn fixture() -> StorageBox {
        StorageBox {
            id: 42,
            name: "my-resource".to_string(),
            storage_box_type: StorageBoxType {
                id: 1,
                name: "bx20".to_string(),
                description: "BX20".to_string(),
                size: 1_073_741_824,
                snapshot_limit: Some(10),
                automatic_snapshot_limit: Some(10),
                subaccounts_limit: 100,
                prices: vec![],
                deprecation: None,
            },
            location: Location {
                id: 1,
                name: "fsn1".to_string(),
                description: "Falkenstein DC Park 1".to_string(),
                country: "DE".to_string(),
                city: "Falkenstein".to_string(),
                latitude: 50.476119,
                longitude: 12.370071,
                network_zone: "eu-central".to_string(),
            },
            access_settings: AccessSettings {
                reachable_externally: false,
                samba_enabled: true,
                ssh_enabled: false,
                webdav_enabled: false,
                zfs_enabled: false,
            },
            snapshot_plan: None,
            protection: Protection { delete: false },
            labels: HashMap::from([("environment".to_string(), "prod".to_string())]),
            status: StorageBoxStatus::Active,
            username: Some("u45321".to_string()),
            server: Some("u1337.your-storagebox.de".to_string()),
            system: Some("FSN1-BX355".to_string()),
            // 100 MiB, 80 MiB, 20 MiB
            stats: Stats {
                size: 104_857_600,
                size_data: 83_886_080,
                size_snapshots: 20_971_520,
            },
            created: "2016-01-30T23:50:00Z".to_string(),
        }
    }

    const LAST_UPDATED: &str = "2026-08-19T12:00:00Z";

    #[test]
    fn extracts_every_known_field() {
        let b = fixture();
        for key in KNOWN_FIELDS {
            assert!(
                extract_field(&b, key, LAST_UPDATED).is_some(),
                "expected a value for known field '{key}'"
            );
        }
    }

    #[test]
    fn nested_field_extracts_correct_value() {
        let b = fixture();
        assert_eq!(
            extract_field(&b, "stats.size", LAST_UPDATED),
            Some(json!(100.0))
        );
        assert_eq!(
            extract_field(&b, "access_settings.samba_enabled", LAST_UPDATED),
            Some(json!(true))
        );
    }

    #[test]
    fn last_updated_extracts_given_timestamp() {
        let b = fixture();
        assert_eq!(
            extract_field(&b, "last_updated", LAST_UPDATED),
            Some(json!(LAST_UPDATED))
        );
    }

    #[test]
    fn unknown_key_returns_none() {
        let b = fixture();
        assert_eq!(extract_field(&b, "does.not.exist", LAST_UPDATED), None);
    }

    #[test]
    fn null_field_returns_none() {
        let mut b = fixture();
        b.username = None;
        assert_eq!(extract_field(&b, "username", LAST_UPDATED), None);
    }

    #[test]
    fn bytes_to_mib_rounds_to_two_decimal_places() {
        assert_eq!(bytes_to_mib(104_857_600), 100.0);
        assert_eq!(bytes_to_mib(1_500_000), 1.43);
    }

    #[test]
    fn to_payload_unwraps_string_without_json_quotes() {
        assert_eq!(to_payload(&json!(LAST_UPDATED)), LAST_UPDATED);
    }

    #[test]
    fn to_payload_leaves_non_string_values_as_is() {
        assert_eq!(to_payload(&json!(100)), "100");
        assert_eq!(to_payload(&json!(true)), "true");
    }
}
