FROM archlinux:latest AS builder

RUN pacman -Sy --noconfirm --needed base-devel rustup

WORKDIR /app
COPY . .

RUN rustup default stable
RUN cargo build --release

FROM archlinux:latest

LABEL org.opencontainers.image.source="https://github.com/bangkahdev/atha"
LABEL org.opencontainers.image.description="A safety and workflow layer for pacman on Arch Linux"

RUN pacman -Sy --noconfirm --needed \
    curl \
    jq \
    git \
    pacman-contrib \
    sudo && \
    pacman -Scc --noconfirm

COPY --from=builder /app/target/release/atha /usr/local/bin/atha

ENTRYPOINT ["atha"]
CMD ["--help"]