FROM rust:1.75 as builder
WORKDIR /app
COPY . .

# ✅ Locked dependency graph (prevents unexpected resolution changes)
RUN cargo build --release --locked

FROM debian:bookworm-slim

# ✅ Minimal runtime user (non-root)
RUN useradd -m -u 1000 amunchain

WORKDIR /home/amunchain

# ✅ Copy only the built binary
COPY --from=builder /app/target/release/amunchain /usr/local/bin/amunchain

USER 1000:1000
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 CMD /usr/bin/env sh -c '[ -z "$AMUNCHAIN_HEALTH_URL" ] && exit 0 || wget -qO- "$AMUNCHAIN_HEALTH_URL" >/dev/null 2>&1'
EXPOSE 30333

ENTRYPOINT ["/usr/local/bin/amunchain"]
CMD ["configs/node.toml"]
