# syntax=docker/dockerfile:1

FROM rust:1.90-slim-bookworm AS builder
WORKDIR /app

# Cache dependencies in their own layer: build with a dummy source
# tree first so `cargo build` fetches and compiles dependencies
# before the real source is copied in.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 app \
    && useradd --system --uid 10001 --gid app --no-create-home --shell /usr/sbin/nologin app

WORKDIR /app
COPY --from=builder --chown=app:app \
    /app/target/release/hetzner-storage-box-to-mqtt \
    /usr/local/bin/hetzner-storage-box-to-mqtt

USER app

# The config file is expected to be bind-mounted to /app/config.toml.
ENTRYPOINT ["/usr/local/bin/hetzner-storage-box-to-mqtt"]
CMD ["--config", "/app/config.toml"]
