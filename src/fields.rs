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
];

/// Extracts a known field's value from a storage box.
/// Returns `None` both for an unknown key and for a field whose value is currently
/// absent (e.g. `username` while the box is still `initializing`) — callers treat
/// both cases the same way: skip publishing it this run.
pub fn extract_field(b: &StorageBox, key: &str) -> Option<Value> {
    match key {
        "id" => Some(json!(b.id)),
        "name" => Some(json!(b.name)),
        "status" => serde_json::to_value(&b.status).ok(),
        "username" => b.username.clone().map(Value::from),
        "server" => b.server.clone().map(Value::from),
        "system" => b.system.clone().map(Value::from),
        "created" => Some(json!(b.created)),
        "stats.size" => Some(json!(b.stats.size)),
        "stats.size_data" => Some(json!(b.stats.size_data)),
        "stats.size_snapshots" => Some(json!(b.stats.size_snapshots)),
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
            stats: Stats {
                size: 100,
                size_data: 80,
                size_snapshots: 20,
            },
            created: "2016-01-30T23:50:00Z".to_string(),
        }
    }

    #[test]
    fn extracts_every_known_field() {
        let b = fixture();
        for key in KNOWN_FIELDS {
            assert!(
                extract_field(&b, key).is_some(),
                "expected a value for known field '{key}'"
            );
        }
    }

    #[test]
    fn nested_field_extracts_correct_value() {
        let b = fixture();
        assert_eq!(extract_field(&b, "stats.size"), Some(json!(100)));
        assert_eq!(
            extract_field(&b, "access_settings.samba_enabled"),
            Some(json!(true))
        );
    }

    #[test]
    fn unknown_key_returns_none() {
        let b = fixture();
        assert_eq!(extract_field(&b, "does.not.exist"), None);
    }

    #[test]
    fn null_field_returns_none() {
        let mut b = fixture();
        b.username = None;
        assert_eq!(extract_field(&b, "username"), None);
    }
}
