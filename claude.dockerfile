FROM debian:trixie-slim

ARG UID=1000
ARG GID=1000
ARG GIT_USER_NAME=""
ARG GIT_USER_EMAIL=""


ENV DISABLE_TELEMETRY=1 \
    DISABLE_ERROR_REPORTING=1 \
    DISABLE_AUTOUPDATER=1 \
    IS_SANDBOX=1 \
    HOME=/home/dev \
    PATH=/home/dev/.local/bin:$PATH

# slim has almost nothing — install what the install script + git need
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl git openssh-client \
      ripgrep fd-find jq git-delta tree less unzip shellcheck \
      build-essential cmake gdb pkg-config \
      python3 python3-venv pipx \
      fzf \
 && rm -rf /var/lib/apt/lists/*

ENV USE_BUILTIN_RIPGREP=0

# Create the non-root user matching your host UID/GID
RUN groupadd -g "$GID" dev \
 && useradd  -m -u "$UID" -g "$GID" -s /bin/bash dev

USER dev

RUN curl -fsSL https://mise.jdx.dev/install.sh | bash
RUN curl -fsSL https://claude.ai/install.sh | bash

RUN git config --global --add safe.directory /workspace \
 && if [ -n "$GIT_USER_NAME" ];  then git config --global user.name  "$GIT_USER_NAME";  fi \
 && if [ -n "$GIT_USER_EMAIL" ]; then git config --global user.email "$GIT_USER_EMAIL"; fi \
 && mkdir -p "$HOME/.ssh" && chmod 700 "$HOME/.ssh" \
 && ssh-keyscan github.com >> "$HOME/.ssh/known_hosts"

RUN echo "alias claude-yolo='claude --dangerously-skip-permissions'" >> "$HOME/.bashrc"

WORKDIR /workspace
