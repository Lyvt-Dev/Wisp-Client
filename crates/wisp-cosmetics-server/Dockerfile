# Built from the repository root: docker build -f crates/wisp-cosmetics-server/Dockerfile .
FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.toml
COPY crates/wisp-compat crates/wisp-compat
COPY crates/wisp-cosmetics crates/wisp-cosmetics
COPY crates/wisp-shop crates/wisp-shop
COPY crates/wisp-cosmetics-server crates/wisp-cosmetics-server
RUN cargo build --release --manifest-path crates/wisp-cosmetics-server/Cargo.toml

FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/crates/wisp-cosmetics-server/target/release/wisp-cosmetics-server /usr/local/bin/
ENV WISP_COSMETICS_DB=/data/cosmetics.sqlite3
CMD ["wisp-cosmetics-server"]
