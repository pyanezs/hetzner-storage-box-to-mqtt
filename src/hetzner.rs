use crate::retry::Retryable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct GetStorageBoxResponse {
    storage_box: StorageBox,
    // "action" intentionally omitted: unused by this app, ignored by serde by default.
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageBox {
    pub id: u64,
    pub name: String,
    pub storage_box_type: StorageBoxType,
    pub location: Location,
    pub access_settings: AccessSettings,
    pub snapshot_plan: Option<SnapshotPlan>,
    pub protection: Protection,
    pub labels: HashMap<String, String>,
    pub status: StorageBoxStatus,
    pub username: Option<String>,
    pub server: Option<String>,
    pub system: Option<String>,
    pub stats: Stats,
    pub created: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageBoxStatus {
    Active,
    Initializing,
    Locked,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Stats {
    pub size: u64,
    pub size_data: u64,
    pub size_snapshots: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AccessSettings {
    pub reachable_externally: bool,
    pub samba_enabled: bool,
    pub ssh_enabled: bool,
    pub webdav_enabled: bool,
    pub zfs_enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Protection {
    pub delete: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageBoxType {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub size: u64,
    pub snapshot_limit: Option<u64>,
    pub automatic_snapshot_limit: Option<u64>,
    pub subaccounts_limit: u64,
    pub prices: Vec<Price>,
    pub deprecation: Option<DeprecationInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Price {
    pub location: String,
    pub price_hourly: Money,
    pub price_monthly: Money,
    pub setup_fee: Money,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Money {
    pub net: String,
    pub gross: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DeprecationInfo {
    pub unavailable_after: String,
    pub announced: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Location {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub country: String,
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
    pub network_zone: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SnapshotPlan {
    pub max_snapshots: u32,
    pub minute: u8,
    pub hour: u8,
    pub day_of_week: Option<u8>,
    pub day_of_month: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct HetznerErrorBody {
    error: HetznerErrorDetail,
}

#[derive(Debug, Deserialize)]
struct HetznerErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug)]
pub enum HetznerError {
    /// A non-2xx response with a parsed `{"error": {...}}` body.
    Api {
        status: u16,
        code: String,
        message: String,
    },
    /// A connection-level failure (timeout, DNS, etc.) — no response body to parse.
    Transport(reqwest::Error),
}

impl std::fmt::Display for HetznerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HetznerError::Api {
                status,
                code,
                message,
            } => {
                write!(f, "Hetzner API ({status}): {code} — {message}")
            }
            HetznerError::Transport(e) => write!(f, "transport error: {e}"),
        }
    }
}

impl std::error::Error for HetznerError {}

impl Retryable for HetznerError {
    fn is_retryable(&self) -> bool {
        match self {
            // 5xx is treated as transient; 4xx will fail identically on every retry.
            HetznerError::Api { status, .. } => *status >= 500,
            HetznerError::Transport(_) => true,
        }
    }
}

pub struct HetznerClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl HetznerClient {
    pub fn new(base_url: String, token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            token,
        }
    }

    async fn get(&self, id: u64) -> Result<reqwest::Response, HetznerError> {
        let url = format!(
            "{}/storage_boxes/{}",
            self.base_url.trim_end_matches('/'),
            id
        );
        self.http
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(HetznerError::Transport)
    }

    pub async fn fetch_storage_box(&self, id: u64) -> Result<StorageBox, HetznerError> {
        let response = self.get(id).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Self::parse_error(status, response).await);
        }
        let body: GetStorageBoxResponse = response.json().await.map_err(HetznerError::Transport)?;
        Ok(body.storage_box)
    }

    /// Fetches the raw response body for a box, bypassing `StorageBox` deserialization
    /// entirely. Used by `--dump-raw` to inspect real API responses independent of
    /// whether the modeled struct above still matches reality.
    pub async fn fetch_raw(&self, id: u64) -> Result<String, HetznerError> {
        let response = self.get(id).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Self::parse_error(status, response).await);
        }
        let text = response.text().await.map_err(HetznerError::Transport)?;
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => Ok(serde_json::to_string_pretty(&value).unwrap_or(text)),
            Err(_) => Ok(text),
        }
    }

    async fn parse_error(status: reqwest::StatusCode, response: reqwest::Response) -> HetznerError {
        let status_code = status.as_u16();
        match response.json::<HetznerErrorBody>().await {
            Ok(body) => HetznerError::Api {
                status: status_code,
                code: body.error.code,
                message: body.error.message,
            },
            Err(_) => HetznerError::Api {
                status: status_code,
                code: "unknown".to_string(),
                message: format!("HTTP {status_code}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn active_box_json() -> serde_json::Value {
        serde_json::json!({
            "storage_box": {
                "id": 42,
                "status": "active",
                "name": "my-resource",
                "username": "u45321",
                "storage_box_type": {
                    "id": 1,
                    "name": "bx20",
                    "description": "BX20",
                    "snapshot_limit": 10,
                    "automatic_snapshot_limit": 10,
                    "subaccounts_limit": 100,
                    "size": 1073741824,
                    "prices": [],
                    "deprecation": null
                },
                "location": {
                    "id": 1,
                    "country": "DE",
                    "city": "Falkenstein",
                    "name": "fsn1",
                    "network_zone": "eu-central",
                    "latitude": 50.476119,
                    "longitude": 12.370071,
                    "description": "Falkenstein DC Park 1"
                },
                "access_settings": {
                    "reachable_externally": false,
                    "samba_enabled": false,
                    "ssh_enabled": false,
                    "webdav_enabled": false,
                    "zfs_enabled": false
                },
                "server": "u1337.your-storagebox.de",
                "system": "FSN1-BX355",
                "stats": { "size": 100, "size_data": 80, "size_snapshots": 20 },
                "labels": { "environment": "prod" },
                "protection": { "delete": false },
                "snapshot_plan": null,
                "created": "2016-01-30T23:50:00Z"
            },
            "action": {
                "id": 13,
                "command": "create",
                "status": "running",
                "progress": 0,
                "started": "2016-01-30T23:50:00Z",
                "finished": null,
                "resources": [{ "id": 42, "type": "storage_box" }],
                "error": null
            }
        })
    }

    fn initializing_box_json() -> serde_json::Value {
        let mut v = active_box_json();
        v["storage_box"]["status"] = serde_json::json!("initializing");
        v["storage_box"]["username"] = serde_json::Value::Null;
        v["storage_box"]["server"] = serde_json::Value::Null;
        v["storage_box"]["system"] = serde_json::Value::Null;
        v
    }

    #[tokio::test]
    async fn fetches_and_parses_an_active_storage_box() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/storage_boxes/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(active_box_json()))
            .mount(&server)
            .await;

        let client = HetznerClient::new(server.uri(), "token".to_string());
        let box_data = client.fetch_storage_box(42).await.unwrap();

        assert_eq!(box_data.id, 42);
        assert_eq!(box_data.name, "my-resource");
        assert_eq!(box_data.status, StorageBoxStatus::Active);
        assert_eq!(box_data.username.as_deref(), Some("u45321"));
        assert_eq!(box_data.stats.size, 100);
    }

    #[tokio::test]
    async fn initializing_box_has_null_fields_as_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/storage_boxes/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(initializing_box_json()))
            .mount(&server)
            .await;

        let client = HetznerClient::new(server.uri(), "token".to_string());
        let box_data = client.fetch_storage_box(42).await.unwrap();

        assert_eq!(box_data.status, StorageBoxStatus::Initializing);
        assert!(box_data.username.is_none());
        assert!(box_data.server.is_none());
        assert!(box_data.system.is_none());
    }

    #[tokio::test]
    async fn unauthorized_response_is_not_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/storage_boxes/42"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": { "code": "unauthorized", "message": "unable to authenticate", "details": null }
            })))
            .mount(&server)
            .await;

        let client = HetznerClient::new(server.uri(), "token".to_string());
        let err = client.fetch_storage_box(42).await.unwrap_err();

        match &err {
            HetznerError::Api { status, code, .. } => {
                assert_eq!(*status, 401);
                assert_eq!(code, "unauthorized");
            }
            _ => panic!("expected HetznerError::Api"),
        }
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn server_error_response_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/storage_boxes/42"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "error": { "code": "timeout", "message": "request timeout", "details": null }
            })))
            .mount(&server)
            .await;

        let client = HetznerClient::new(server.uri(), "token".to_string());
        let err = client.fetch_storage_box(42).await.unwrap_err();

        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn missing_required_field_fails_to_deserialize() {
        let server = MockServer::start().await;
        let mut body = active_box_json();
        body["storage_box"].as_object_mut().unwrap().remove("stats");
        Mock::given(method("GET"))
            .and(path("/storage_boxes/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = HetznerClient::new(server.uri(), "token".to_string());
        let result = client.fetch_storage_box(42).await;

        assert!(result.is_err());
    }
}
