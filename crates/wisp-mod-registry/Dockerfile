# Built from the repository root: docker build -f crates/wisp-mod-registry/Dockerfile .
FROM rust:1-slim AS build
WORKDIR /src
# The root manifest only for the shared crate's workspace settings (edition, lints).
COPY Cargo.toml Cargo.toml
COPY crates/wisp-mods crates/wisp-mods
# wisp-mods' optional dependency: cargo needs its manifest even with the feature off.
COPY crates/wisp-shader-api crates/wisp-shader-api
COPY crates/wisp-mod-registry crates/wisp-mod-registry
RUN cargo build --release --locked --manifest-path crates/wisp-mod-registry/Cargo.toml

FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/crates/wisp-mod-registry/target/release/wisp-mod-registry /usr/local/bin/
ENV REGISTRY_DB=/data/registry.sqlite3
# Railway mounts the ratings' volume at /data owned by root: hand it to nobody, then run as nobody (setpriv is
# util-linux's, in every Debian image).
CMD ["sh", "-c", "mkdir -p /data && chown nobody:nogroup /data && exec setpriv --reuid=nobody --regid=nogroup --clear-groups wisp-mod-registry"]
