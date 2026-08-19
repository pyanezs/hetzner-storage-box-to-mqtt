# Configuring the app

The app reads a TOML config file — `config.toml` by default, created by
copying `config.example.toml` (see [Dev environment setup](../README.md#dev-environment-setup)).

**Config file location**, in order of precedence:

1. `--config <path>` CLI flag
2. `HETZNER_MQTT_CONFIG` environment variable
3. `./config.toml` (default)

## `[general]`

| Field | Type | Required | Default |
|---|---|---|---|
| `retry_enabled` | bool | yes | — |
| `log_level` | string | no | `info` |
| `run_times` | list of strings | no | `[]` |

`retry_enabled` controls whether to retry a failed Hetzner API call
(5xx / connection errors only, not 4xx) or a failed MQTT publish, using
a fixed backoff: 2, 4, 8, 16, 32, 64 seconds, then stop.

`log_level` must be one of `trace`, `debug`, `info`, `warn`, `error`
(case-insensitive). It's ignored if the `RUST_LOG` environment variable
is set — `RUST_LOG` always takes precedence and supports tracing's full
per-module directive syntax (e.g. `hetzner_storage_box_to_mqtt=debug`).

`run_times` is a list of daily clock times, each in 24-hour `HH:MM`
format (e.g. `"03:00"`, `"15:30"`), interpreted in the system's local
timezone (not UTC). The first cycle runs immediately on startup; after
that, a cycle runs at each listed time, every day. Each entry must be a
valid `HH:MM` time. A cycle's failure is logged but doesn't stop the
process — the next scheduled cycle still runs. If empty or omitted, the
app runs only the one startup cycle and exits, instead of staying
resident.

## `[hetzner]`

| Field | Type | Required | Default |
|---|---|---|---|
| `api_token` | string | yes (or via env) | — |
| `api_base_url` | string | no | `https://api.hetzner.com/v1` |

`api_token` can be omitted from the file entirely if `HETZNER_API_TOKEN`
is set — the environment variable takes precedence over the file when
both are present, and config loading fails if neither supplies a
non-empty value.

## `[mqtt]`

| Field | Type | Required | Purpose |
|---|---|---|---|
| `host` | string | yes | broker hostname |
| `port` | number | yes | broker port |
| `username` | string | yes | MQTT auth username |
| `password` | string | yes (or via env) | MQTT auth password |
| `client_id` | string | yes | MQTT client id |
| `base_topic` | string | yes | state topic prefix: `<base_topic>/<box id>/<field>/state` |
| `discovery_prefix` | string | yes | Home Assistant discovery topic prefix |
| `tls` | bool | yes | see warning below |

`password` can also be set via the `MQTT_PASSWORD` environment variable,
which takes precedence over the file if set.
Dots in field names are replaced with underscores in topic paths
(e.g. `stats.size` becomes `.../stats_size/state`).

> [!WARNING]
> `mqtt.tls` is accepted by the schema but not functionally implemented yet.
> The app always connects in plaintext and only logs a warning if
> `tls = true`.

## `[[storage_box]]`

One block per Hetzner Storage Box to track. A box with no matching entry,
or `publish = false`, is never fetched or published.

| Field | Type | Required | Default |
|---|---|---|---|
| `id` | number | yes | — |
| `publish` | bool | no | `false` |
| `fields` | list of strings | no (non-empty required if `publish = true`) | `[]` |
| `field_meta` | table | no | `{}` |
| `alias` | string | no | Hetzner-reported name |

- `id` is the numeric Hetzner Cloud resource id (not the name) — find it via
  the Hetzner Cloud Console.
- `alias` overrides the Home Assistant device display name for this box.
  When omitted, the Hetzner API-reported name is used instead.
  Must not be an empty string if set.
- `fields` selects which values to publish, as dotted-path strings. Every
  entry must be one of the paths listed in `src/fields.rs`'s `KNOWN_FIELDS`
  — an unknown field name fails config validation with an error naming the
  box and field, rather than being silently ignored. `KNOWN_FIELDS` is a
  curated subset of what the Hetzner API exposes (pricing/deprecation/
  snapshot-plan details are left out as not useful for a home dashboard).
- `[storage_box.field_meta]` is optional, per-field Home Assistant metadata
  (`friendly_name`, `unit_of_measurement`, `device_class`), keyed by the
  same dotted-path string used in `fields`. A field left out here still
  publishes fine — it falls back to a title-cased version of its field
  path as its entity name (e.g. `stats.size` -> `Stats Size`), with no
  unit or device class. Keys must also exist in `KNOWN_FIELDS`.
- `last_updated` is always published for every published box, regardless
  of what's listed in `fields` — an RFC3339 UTC timestamp
  (e.g. `2026-08-19T12:34:56Z`) captured once per run, right before
  publishing to MQTT. It defaults to Home Assistant's `timestamp` device
  class, and can still be given a `friendly_name` or overridden
  `device_class` via `field_meta` like any other field.
- `stats.size`, `stats.size_data`, and `stats.size_snapshots` are
  published in MiB, converted from the raw bytes the Hetzner API reports
  (rounded to 2 decimal places). They default to
  `unit_of_measurement = "MiB"` without needing a `field_meta` entry —
  set one explicitly to override it (e.g. back to `"B"`, or up to
  `"GiB"`).

## Environment variable overrides

| Variable | Overrides |
|---|---|
| `HETZNER_API_TOKEN` | `hetzner.api_token` |
| `MQTT_PASSWORD` | `mqtt.password` |
| `HETZNER_MQTT_CONFIG` | config file path (lower precedence than `--config`) |

## CLI flags

| Flag | Effect |
|---|---|
| `--config <path>` | overrides the config file location |

Flags are parsed manually (no `clap`) — an unrecognized flag is silently
ignored, and there's no `--help`/`--version`.
