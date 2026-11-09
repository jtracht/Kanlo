FROM debian:13-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl build-essential gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf libc6-dev-arm64-cross libc6-dev-armhf-cross file python3 && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh && sh /tmp/rustup.sh -y --profile minimal --default-toolchain 1.99.1 && rm /tmp/rustup.sh
ENV PATH=/root/.cargo/bin:$PATH
RUN rustup target add aarch64-unknown-linux-gnu armv7-unknown-linux-gnueabihf
WORKDIR /src
RUN useradd --system --user-group --home-dir /var/lib/kanlo --no-create-home --shell /usr/sbin/nologin kanlo
