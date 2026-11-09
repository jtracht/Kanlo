FROM debian:13-slim
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git python3 qemu-utils qemu-system-x86 qemu-system-arm \
    ovmf qemu-efi-aarch64 util-linux parted e2fsprogs dosfstools xz-utils \
    shellcheck python3-pygame cage seatd fonts-dejavu-core && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /src
