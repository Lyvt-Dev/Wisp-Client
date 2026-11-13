# The Play with friends relay, for anyone to run (docs/play-with-friends.md, "Running your own relay").
# Build context: the repository root (the workspace's manifests and lockfile):
#   docker build -f crates/wisp-relay/Dockerfile -t wisp-relay .
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p wisp-relay \
    && cp target/release/wisp-relay /wisp-relay

FROM debian:bookworm-slim
COPY --from=build /wisp-relay /usr/local/bin/wisp-relay
ENV PORT=7000
EXPOSE 7000
USER nobody
CMD ["wisp-relay"]
