# setup lazymc versions
ARG LAZYMC_VERSION=0.2.11
ARG LAZYMC_LEGACY_VERSION=0.2.10

# set up rust
FROM --platform=$BUILDPLATFORM rust:1.90 AS rust-setup
ARG TARGETARCH
RUN <<EOF
  echo Running build for $TARGETARCH
  if [ "$TARGETARCH" = "amd64" ]; then
    echo x86_64-unknown-linux-musl > /rust-arch
  elif [ "$TARGETARCH" = "arm64" ]; then
    echo aarch64-unknown-linux-musl > /rust-arch
    mkdir -p /.cargo/
    echo [target.aarch64-unknown-linux-musl] >> /.cargo/config.toml
    echo linker = \"aarch64-linux-gnu-gcc\" >> /.cargo/config.toml
  fi
EOF
RUN rustup target add "$(cat /rust-arch)"
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates
RUN apt-get update && apt-get install -y pkg-config libssl-dev crossbuild-essential-arm64 crossbuild-essential-armhf

# build lazymc
FROM --platform=$BUILDPLATFORM rust-setup AS lazymc-builder
WORKDIR /usr/src/lazymc
ARG LAZYMC_VERSION
ENV LAZYMC_VERSION=$LAZYMC_VERSION
RUN git clone --branch v$LAZYMC_VERSION https://github.com/timvisee/lazymc .
RUN cargo build --target "$(cat /rust-arch)" --release
RUN mv /usr/src/lazymc/target/"$(cat /rust-arch)" /usr/src/lazymc/target/output_final

# build lazymc-legacy
FROM --platform=$BUILDPLATFORM rust-setup AS lazymc-legacy-builder
WORKDIR /usr/src/lazymc
ARG LAZYMC_LEGACY_VERSION
ENV LAZYMC_LEGACY_VERSION=$LAZYMC_LEGACY_VERSION
RUN git clone --branch v$LAZYMC_LEGACY_VERSION https://github.com/timvisee/lazymc .
RUN cargo build --target "$(cat /rust-arch)" --release
RUN mv /usr/src/lazymc/target/"$(cat /rust-arch)" /usr/src/lazymc/target/output_final

# build this app
FROM --platform=$BUILDPLATFORM rust-setup AS app-builder
WORKDIR /usr/src/lazymc-docker-proxy
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# --locked is omitted so cargo can reconcile the lock file after dependency changes.
# Once a successful build produces a fresh Cargo.lock, commit it and restore --locked.
RUN cargo build --target "$(cat /rust-arch)" --release
RUN mv /usr/src/lazymc-docker-proxy/target/"$(cat /rust-arch)" /usr/src/lazymc-docker-proxy/target/output_final

# final image — Alpine so that the `ssh` client binary is available for remote PC management
FROM alpine:3.20

RUN apk add --no-cache openssh-client ca-certificates

# setup lazymc version env vars (used by config.rs to select the right binary)
ARG LAZYMC_VERSION
ENV LAZYMC_VERSION=$LAZYMC_VERSION
ARG LAZYMC_LEGACY_VERSION
ENV LAZYMC_LEGACY_VERSION=$LAZYMC_LEGACY_VERSION

# Copy the compiled binaries
COPY --from=lazymc-builder /usr/src/lazymc/target/output_final/release/lazymc /usr/local/bin/lazymc
COPY --from=lazymc-legacy-builder /usr/src/lazymc/target/output_final/release/lazymc /usr/local/bin/lazymc-legacy
COPY --from=app-builder /usr/src/lazymc-docker-proxy/target/output_final/release/lazymc-docker-proxy /usr/local/bin/lazymc-docker-proxy

# Initialise health state file
RUN mkdir -p /app && echo "STARTING" > /app/health

WORKDIR /app

HEALTHCHECK --start-period=1m --interval=5s --retries=24 CMD ["lazymc-docker-proxy", "--health"]

ENTRYPOINT ["lazymc-docker-proxy"]
