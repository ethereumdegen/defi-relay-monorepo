FROM rust:bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
 && rm -rf /var/lib/apt/lists/*

COPY . ./
RUN cargo build --release --locked

# --- Stage 2 ---
FROM debian:bookworm-slim

ENV PORT=8080

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/x402-rs /usr/local/bin/x402-rs
COPY --from=builder /app/config.json /app/config.json

EXPOSE $PORT
ENV RUST_LOG=info

ENTRYPOINT ["x402-rs"]
