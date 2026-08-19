# hetzner-storage-box-to-mqtt

Pulls Storage Box information from the Hetzner Cloud API
and publishes it to MQTT, with Home Assistant MQTT Discovery support.

```
+-------------+               +-----+                   +--------------------+       +----------------+
| Hetzner API |-- HTTP GET -->| App |-- MQTT publish -->| Broker (Mosquitto) |------>| Home Assistant |
+-------------+               +-----+                   +--------------------+       +----------------+
```

The app is one-shot: it fetches configured storage boxes once, publishes once, and exits.
It's meant to be triggered externally (cron, systemd timer), not run as a daemon.

## Table of contents

- [Requirements](#requirements)
- [Dev environment setup](#dev-environment-setup)
  - [Quick start](#quick-start)
  - [Detailed setup](#detailed-setup)
- [Configuring the app](docs/configuration.md)
- [Common tasks](#common-tasks)
- [Using the Claude Code sandbox](#using-the-claude-code-sandbox)
  - [VM lifecycle](#vm-lifecycle)
  - [Giving the sandboxed Claude access to `mise` tasks](#giving-the-sandboxed-claude-access-to-mise-tasks)
- [Links](#links)

## Requirements

- `mise` — manages the Rust toolchain and every task in this repo (see [Links](#links)).
  Everything below is run through it; you don't need `rustup`/`cargo` installed separately.

## Dev environment setup

### Quick start

Install `mise` and the pinned toolchain:

```
mise trust    # trust this repo's mise.toml
mise install  # install the pinned Rust toolchain and other tools
```

Optional — if you also want to use the [Claude Code sandbox](#using-the-claude-code-sandbox),
build the sandbox VM image, create (but don't start) the VM,
and wire up the `.mcp.json` file the sandboxed Claude uses to reach the host:

```
mise run docker-build   # build the claude-box sandbox image
mise run vm-create      # create the VM from that image (doesn't start it)
mise run mcp-config     # render .mcp.json from .mcp.template.json
```

### Detailed setup

1. Install `mise` (see [Links](#links)), then from the repo root:

   ```
   mise trust
   mise install
   ```

   This installs the pinned Rust toolchain (with `clippy`/`rustfmt` components),
   `rust-analyzer`, `prek`, `typos`, and the other tools listed in `mise.toml`.

2. Copy the example config and fill in real values:

   ```
   cp config.example.toml config.toml
   ```

   `config.toml` is gitignored — it holds your Hetzner API token and MQTT credentials,
   and is never committed.
   Both secrets can also be supplied via environment variables instead
   (`HETZNER_API_TOKEN`, `MQTT_PASSWORD`), which take precedence over the file if set.
   See the comments in `config.example.toml` for the full schema,
   and `src/fields.rs`'s `KNOWN_FIELDS` for the list of selectable field paths.

3. Verify the setup:

   ```
   mise run cargo-build
   mise run cargo-test
   mise run lint
   ```

## Configuring the app

See [`docs/configuration.md`](docs/configuration.md) for the config file
location and precedence, the full `[general]`/`[hetzner]`/`[mqtt]`/
`[[storage_box]]` schema, environment variable overrides, and CLI flags.

## Common tasks

Run `mise tasks` for the full list. The ones you'll use day to day:

| Task | What it does |
|---|---|
| `mise run cargo-build` | `cargo build` |
| `mise run cargo-test` | `cargo test` (unit tests + `wiremock`-based Hetzner API tests) |
| `mise run run -- <args>` | `cargo run --`, e.g. `mise run run -- --config /path/to/config.toml` |
| `mise run lint` | `cargo fmt --check` then `cargo clippy` |
| `mise run cargo-fmt` | `cargo fmt` (writes changes) |
| `mise run prek-run` | Runs the pre-commit-style hooks in `prek.toml` against staged files |

`prek-run` must pass before creating a commit — this is enforced by convention (see `CLAUDE.md`),
not by a git hook, so run it yourself before committing.

## Using the Claude Code sandbox

This repo includes a self-contained sandbox for running Claude Code
against this project without giving it direct access to your host machine:
a Docker image (`claude.dockerfile`) run inside a smolvm micro-VM (see [Links](#links)),
configured by `Smolfile`, with your project directory mounted in
and a narrow, explicit egress allowlist.

### VM lifecycle

```
mise run vm-up        # build the image (if needed), create and start the VM, open a shell in it
mise run vm-shell     # open a shell in an already-running VM (starts it first if needed)
mise run vm-stop      # stop the VM without deleting it
mise run vm-destroy   # stop and delete the VM
mise run vm-recreate  # destroy and recreate the VM from scratch
```

Inside the VM, the project directory is mounted live at `/workspace` in both directions,
and `claude` is preinstalled — run it there to work on this project inside the sandbox.
A `claude-yolo` alias is also predefined in the VM's `~/.bashrc`,
running `claude --dangerously-skip-permissions`.
Skipping permission prompts is reasonable here since the VM is already sandboxed from the host.
The alias is baked into the image, so an existing VM needs `mise run docker-rebuild`
(or `mise run vm-recreate`) before it picks up.

### Giving the sandboxed Claude access to `mise` tasks

The sandbox's network is locked down to the hosts listed in `Smolfile`'s `[network] allow_hosts`
(plus a narrow CIDR for reaching the host). It has no direct access to your host's `mise`/`cargo`
installation or general network — including crates.io, which `cargo build`/`test` need.
To work around this, `mise mcp` (mise's own MCP server) runs on the **host**
and is exposed to the sandboxed Claude as an MCP tool server:

```
+----------------------------------------------------------------------------+
|                             VM ($SMOL_MACHINE)                             |
+----------------------------------------------------------------------------+
| claude / claude-yolo MCP client                                            |
|                      .mcp.json -> host.smolvm.internal:$MCP_MISE_PORT/sse  |
| requests:            cargo-build, cargo-test, lint, prek-run               |
+----------------------------------------------------------------------------+
                                      |
                      egress allowed only to:
                      Smolfile [network] allow_hosts
                      + host.smolvm.internal (narrow CIDR)
                      no crates.io / general internet
                                      |
== network boundary (smolvm firewall) =====================================
                                      |
                                      v
+----------------------------------------------------------------------------+
|                                    HOST                                    |
+----------------------------------------------------------------------------+
| mise run mcp-proxy   SSE proxy on 0.0.0.0:$MCP_MISE_PORT, wraps `mise mcp` |
| mise mcp (server)    receives the request and RUNS the task here:          |
|                      cargo build / test / fmt / clippy, prek run           |
|                      using the host's rust toolchain + full network        |
+----------------------------------------------------------------------------+
```

The VM only sends the MCP request over that SSE connection —
`mise mcp` on the host is what actually executes `cargo build`/`test`/etc.,
using the host's toolchain and network access,
since the VM's egress is firewalled off from crates.io.

1. On the host, run `mise run mcp-proxy`.
   This starts `mise mcp` behind an SSE proxy on `0.0.0.0:$MCP_MISE_PORT` (default `8765`),
   reachable from inside the VM via `host.smolvm.internal`.
2. Run `mise run mcp-config` to render `.mcp.json` (gitignored) from `.mcp.template.json`,
   substituting in `MCP_MISE_URL`.
   `mise.toml` defaults `MCP_MISE_URL` to `http://host.smolvm.internal:$MCP_MISE_PORT/sse`,
   so this works out of the box for the default single-host setup.
   Only add a `mise.local.toml` (gitignored, not tracked) with an `[env]` override
   if `mcp-proxy` is reachable at a different URL.

Once connected, tasks like `cargo-build`/`cargo-test`/`prek-run` run through the `mise` MCP tools
from inside the sandbox, with real network access on the host side —
this is how `cargo build`/`test`/`lint` were actually run and verified during development,
since the sandbox itself can't reach crates.io directly.

> [!IMPORTANT]
> Claude Code's MCP client always probes remote HTTP/SSE servers for OAuth support.
> `mcp-proxy` doesn't implement any OAuth endpoints, so it 404s those probes —
> and depending on the Claude Code version, the client can fail to handle that gracefully,
> breaking the whole MCP session (every task call then fails with a generic
> `MCP error -32602: Invalid request parameters`, not just the auth step).
> If this happens, running `/mcp` and reconnecting has been enough to recover in practice.
> See the upstream tracking issue in [Links](#links).

## Links

- [mise](https://mise.jdx.dev/) — manages the Rust toolchain and every task in this repo
- [smolvm](https://github.com/smol-ai/smolvm) — the micro-VM used for the Claude Code sandbox
- [anthropics/claude-code#46640](https://github.com/anthropics/claude-code/issues/46640) —
  upstream tracking issue for the MCP OAuth-probe bug described above
