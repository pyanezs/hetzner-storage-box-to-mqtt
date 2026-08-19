use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub general: General,
    pub hetzner: Hetzner,
    pub mqtt: Mqtt,
    #[serde(default)]
    pub storage_box: Vec<StorageBoxConfig>,
}

#[derive(Debug, Deserialize)]
pub struct General {
    pub retry_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct Hetzner {
    #[serde(default)]
    pub api_token: String,
    #[serde(default = "default_api_base_url")]
    pub api_base_url: String,
}

fn default_api_base_url() -> String {
    "https://api.hetzner.com/v1".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct Mqtt {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub base_topic: String,
    pub discovery_prefix: String,
    pub tls: bool,
}

#[derive(Debug, Deserialize)]
pub struct StorageBoxConfig {
    pub id: u64,
    #[serde(default)]
    pub publish: bool,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub field_meta: HashMap<String, FieldMeta>,
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FieldMeta {
    pub friendly_name: Option<String>,
    pub unit_of_measurement: Option<String>,
    pub device_class: Option<String>,
}

/// Resolves the config file path: CLI arg > `HETZNER_MQTT_CONFIG` env var > `./config.toml`.
pub fn resolve_config_path(cli_arg: Option<String>) -> PathBuf {
    cli_arg
        .map(PathBuf::from)
        .or_else(|| std::env::var("HETZNER_MQTT_CONFIG").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

pub fn load_from_path(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    let mut config: Config =
        toml::from_str(&text).with_context(|| format!("parsing config file {}", path.display()))?;
    apply_env_overrides(&mut config, &|key| std::env::var(key).ok());
    validate(&config)?;
    Ok(config)
}

/// `env` is injected (rather than reading `std::env` directly) so this is testable
/// without mutating real process-global environment variables.
fn apply_env_overrides(config: &mut Config, env: &dyn Fn(&str) -> Option<String>) {
    if let Some(token) = env("HETZNER_API_TOKEN") {
        config.hetzner.api_token = token;
    }
    if let Some(password) = env("MQTT_PASSWORD") {
        config.mqtt.password = password;
    }
}

fn validate(config: &Config) -> Result<()> {
    if config.hetzner.api_token.trim().is_empty() {
        bail!(
            "hetzner.api_token must not be empty (set it in the config file or HETZNER_API_TOKEN)"
        );
    }
    if config.mqtt.host.trim().is_empty() {
        bail!("mqtt.host must not be empty");
    }
    for sb in &config.storage_box {
        if sb.publish && sb.fields.is_empty() {
            bail!(
                "storage_box {} has publish = true but no fields configured",
                sb.id
            );
        }
        if let Some(alias) = &sb.alias
            && alias.trim().is_empty()
        {
            bail!("storage_box {}: alias must not be empty if set", sb.id);
        }
        for field in &sb.fields {
            if !crate::fields::KNOWN_FIELDS.contains(&field.as_str()) {
                bail!("storage_box {}: unknown field '{field}' in `fields`", sb.id);
            }
        }
        for key in sb.field_meta.keys() {
            if !crate::fields::KNOWN_FIELDS.contains(&key.as_str()) {
                bail!(
                    "storage_box {}: unknown field '{key}' in `field_meta`",
                    sb.id
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[general]
retry_enabled = true

[hetzner]
api_token = "secret-token"

[mqtt]
host = "mqtt.example.local"
port = 1883
username = "mqtt-user"
password = "mqtt-pass"
client_id = "hetzner-storage-box-to-mqtt"
base_topic = "hetzner_storage_box"
discovery_prefix = "homeassistant"
tls = false

[[storage_box]]
id = 12345678
publish = true
fields = ["name", "stats.size"]
alias = "Backup Box"

[storage_box.field_meta]
"stats.size" = { friendly_name = "Used Size", unit_of_measurement = "B" }

[[storage_box]]
id = 87654321
publish = false
"#;

    #[test]
    fn parses_sample_config() {
        let config: Config = toml::from_str(SAMPLE).unwrap();
        assert!(config.general.retry_enabled);
        assert_eq!(config.hetzner.api_token, "secret-token");
        assert_eq!(config.hetzner.api_base_url, "https://api.hetzner.com/v1");
        assert_eq!(config.mqtt.host, "mqtt.example.local");
        assert_eq!(config.storage_box.len(), 2);
        assert_eq!(config.storage_box[0].id, 12345678);
        assert!(config.storage_box[0].publish);
        assert_eq!(
            config.storage_box[0].field_meta["stats.size"]
                .friendly_name
                .as_deref(),
            Some("Used Size")
        );
        assert_eq!(config.storage_box[0].alias.as_deref(), Some("Backup Box"));
        assert!(!config.storage_box[1].publish);
    }

    #[test]
    fn alias_defaults_to_none_when_absent() {
        let config: Config = toml::from_str(SAMPLE).unwrap();
        assert!(config.storage_box[1].alias.is_none());
    }

    #[test]
    fn rejects_empty_alias() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        config.storage_box[0].alias = Some(String::new());
        assert!(validate(&config).is_err());
    }

    #[test]
    fn env_override_takes_precedence_over_config_file() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        apply_env_overrides(&mut config, &|key| match key {
            "HETZNER_API_TOKEN" => Some("from-env".to_string()),
            "MQTT_PASSWORD" => Some("also-from-env".to_string()),
            _ => None,
        });
        assert_eq!(config.hetzner.api_token, "from-env");
        assert_eq!(config.mqtt.password, "also-from-env");
    }

    #[test]
    fn env_override_absent_keeps_config_file_value() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        apply_env_overrides(&mut config, &|_| None);
        assert_eq!(config.hetzner.api_token, "secret-token");
    }

    #[test]
    fn rejects_empty_api_token() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        config.hetzner.api_token = String::new();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn api_token_can_be_omitted_from_file_and_supplied_via_env() {
        const NO_TOKEN: &str = r#"
[general]
retry_enabled = true

[hetzner]

[mqtt]
host = "mqtt.example.local"
port = 1883
username = "mqtt-user"
password = "mqtt-pass"
client_id = "hetzner-storage-box-to-mqtt"
base_topic = "hetzner_storage_box"
discovery_prefix = "homeassistant"
tls = false
"#;
        let mut config: Config = toml::from_str(NO_TOKEN).unwrap();
        assert_eq!(config.hetzner.api_token, "");
        assert!(validate(&config).is_err());

        apply_env_overrides(&mut config, &|key| match key {
            "HETZNER_API_TOKEN" => Some("from-env".to_string()),
            _ => None,
        });
        assert_eq!(config.hetzner.api_token, "from-env");
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn rejects_empty_mqtt_host() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        config.mqtt.host = String::new();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn rejects_publish_true_with_no_fields() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        config.storage_box[0].fields.clear();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn rejects_unknown_field_name() {
        let mut config: Config = toml::from_str(SAMPLE).unwrap();
        config.storage_box[0]
            .fields
            .push("not.a.real.field".to_string());
        assert!(validate(&config).is_err());
    }
}
