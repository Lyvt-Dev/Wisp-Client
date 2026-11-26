# Built from the repository root: docker build -f crates/wisp-telemetry-server/Dockerfile .
FROM rust:1-slim AS build
WORKDIR /src
# The root manifest only for the shared crate's workspace settings (edition, lints).
COPY Cargo.toml Cargo.toml
COPY crates/wisp-telemetry crates/wisp-telemetry
COPY crates/wisp-telemetry-server crates/wisp-telemetry-server
RUN cargo build --release --locked --manifest-path crates/wisp-telemetry-server/Cargo.toml

FROM debian:stable-slim
COPY --from=build /src/crates/wisp-telemetry-server/target/release/wisp-telemetry-server /usr/local/bin/
ENV WISP_TELEMETRY_DB=/data/telemetry.sqlite3 WISP_TELEMETRY_TRUST_PROXY=1
CMD ["wisp-telemetry-server"]
