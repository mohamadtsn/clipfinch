# syntax=docker/dockerfile:1
ARG UBUNTU_IMAGE=ubuntu:26.04@sha256:2260313b31c8c011cd2eebe728008efac1b3982be73eb71348ea2648d2c0e09b
FROM ${UBUNTU_IMAGE} AS dev
ARG RUST_VERSION=1.93.1ubuntu1
ARG APP_UID=1000
ARG APP_GID=1000
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates build-essential pkg-config \
    rustc=${RUST_VERSION} cargo=${RUST_VERSION} \
    rustfmt=${RUST_VERSION} rust-clippy=${RUST_VERSION} \
    libgtk-4-dev libdbus-1-dev libx11-dev libxtst-dev libxkbcommon-dev \
    dbus-x11 xvfb xauth python3 gnome-settings-daemon-common \
    && rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y --no-install-recommends gsettings-desktop-schemas \
    && rm -rf /var/lib/apt/lists/*
COPY assets/fonts/*.ttf /usr/local/share/fonts/clipfinch/
COPY assets/fonts/*OFL.txt /usr/share/doc/clipfinch/fonts/
RUN fc-cache -f
RUN groupadd --non-unique --gid ${APP_GID} clipfinch \
    && useradd --non-unique --uid ${APP_UID} --gid clipfinch --create-home clipfinch \
    && mkdir -p /workspace/target /home/clipfinch/.cargo \
    && chown -R ${APP_UID}:${APP_GID} /workspace /home/clipfinch
ENV CARGO_HOME=/home/clipfinch/.cargo \
    CARGO_BUILD_JOBS=2 \
    CARGO_PROFILE_DEV_DEBUG=0
WORKDIR /workspace
USER clipfinch
CMD ["cargo", "test", "--locked", "-p", "clipfinch-core", "-p", "clipfinch-linux"]

FROM dev AS build
ARG APP_UID=1000
ARG APP_GID=1000
COPY --chown=${APP_UID}:${APP_GID} Cargo.toml Cargo.lock ./
COPY --chown=${APP_UID}:${APP_GID} crates ./crates
RUN --mount=type=cache,id=clipfinch-cargo-${APP_UID}-${APP_GID},target=/home/clipfinch/.cargo,uid=${APP_UID},gid=${APP_GID} \
    --mount=type=cache,id=clipfinch-target-${APP_UID}-${APP_GID},target=/workspace/target,uid=${APP_UID},gid=${APP_GID} \
    cargo build --locked --release && cp target/release/clipfinch /home/clipfinch/clipfinch

FROM dev AS gui-test
USER root
RUN apt-get update && apt-get install -y --no-install-recommends at-spi2-core openbox \
    && rm -rf /var/lib/apt/lists/*
USER clipfinch

FROM ${UBUNTU_IMAGE} AS runtime
ARG APP_UID=1000
ARG APP_GID=1000
RUN apt-get update && apt-get install -y --no-install-recommends \
    libgtk-4-1 libdbus-1-3 libx11-6 libxtst6 libxkbcommon0 \
    libgl1 libegl1 libglib2.0-bin dbus-bin gnome-settings-daemon-common \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --non-unique --gid ${APP_GID} clipfinch \
    && useradd --non-unique --uid ${APP_UID} --gid clipfinch --create-home clipfinch \
    && mkdir -p /run/user/${APP_UID} /tmp/.X11-unix \
    && chown ${APP_UID}:${APP_GID} /run/user/${APP_UID} \
    && chmod 700 /run/user/${APP_UID}
RUN apt-get update && apt-get install -y --no-install-recommends gsettings-desktop-schemas \
    && rm -rf /var/lib/apt/lists/*
COPY assets/fonts/*.ttf /usr/local/share/fonts/clipfinch/
COPY assets/fonts/*OFL.txt /usr/share/doc/clipfinch/fonts/
RUN fc-cache -f
COPY --from=build /home/clipfinch/clipfinch /usr/local/bin/clipfinch
COPY assets/icon.png /usr/share/icons/hicolor/512x512/apps/clipfinch.png
COPY --chmod=755 scripts/container-entrypoint.sh /usr/local/bin/clipfinch-entrypoint
USER clipfinch
ENTRYPOINT ["/usr/local/bin/clipfinch-entrypoint"]
CMD ["--show"]
