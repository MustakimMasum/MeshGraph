FROM rust:1-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

WORKDIR /app
COPY Cargo.toml Cargo.lock index.html ./
COPY src ./src
COPY public ./public

RUN trunk build --release \
    && cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/meshgraph_gateway /usr/local/bin/meshgraph_gateway
COPY --from=builder /app/dist ./dist

EXPOSE 3000
CMD ["meshgraph_gateway"]
