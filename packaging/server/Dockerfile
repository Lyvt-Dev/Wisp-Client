# wisp-server in a container (docs/server.md, section 3). Build context: the repository root:
#   docker build -f packaging/server/Dockerfile -t wisp-server .
#
# The image holds none of Minecraft's files. Prepare them on the host from your own Minecraft 26.3
# (wisp-prepare --prepare, see packaging/PLAY.txt) and mount the folder it writes, read-only:
#   docker run -d --name wisp -p 25565:25565 --stop-timeout 180 \
#     -v wisp-data:/data \
#     -v /path/to/.local/wisp-client/26.3:/opt/wisp/.local/wisp-client/26.3:ro \
#     -e WISP_EULA=true -e WISP_WHITELIST=true -e WISP_PLAYERS=Alex,Steve -e WISP_OP=Alex \
#     wisp-server
#
# WISP_EULA=true accepts the Minecraft EULA (https://aka.ms/MinecraftEULA) for this server. The first
# start writes /data/server.toml from WISP_* (EULA, WORLD, SEED, PORT, WHITELIST, PLAYERS, OP); later
# starts use the file. Manage it with `docker exec wisp wisp-server status|cmd|backup|logs ...`.
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
# The server build of the game (no sound, no controllers: it needs neither ALSA nor udev) as wisp-engine.
RUN cargo build --release --locked -p wisp-client --no-default-features --bin wisp-client \
    && cargo build --release --locked -p wisp-server --bin wisp-server \
    && mkdir /out && cp target/release/wisp-client /out/wisp-engine && cp target/release/wisp-server /out/

FROM debian:bookworm-slim
RUN useradd --system --home-dir /data --shell /usr/sbin/nologin wisp \
    && mkdir -p /data /opt/wisp/.local/wisp-client \
    && chown wisp:wisp /data
COPY --from=build /out/ /opt/wisp/
ENV PATH=/opt/wisp:$PATH \
    WISP_NO_AUDIO=1
WORKDIR /data
VOLUME /data
EXPOSE 25565
USER wisp
# SIGTERM makes wisp-server stop the game cleanly (it saves first): give it --stop-timeout 180.
CMD ["sh", "-c", "test -f server.toml || wisp-server init --yes || exit 1; exec wisp-server run"]
