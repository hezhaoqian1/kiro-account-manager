FROM node:22-bookworm AS web
WORKDIR /workspace
COPY package.json bun.lock ./
RUN npm install --ignore-scripts --no-audit --no-fund
COPY index.html vite.config.js tsconfig.json tsconfig.node.json jsconfig.json components.json ./
COPY public public
COPY locales locales
COPY src src
RUN npm run build

FROM rust:1.88-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev librsvg2-dev build-essential \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace/src-tauri
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./
RUN mkdir -p src && printf 'fn main() {}\n' > src/main.rs && cargo fetch
COPY src-tauri/src src
COPY src-tauri/build.rs ./
COPY src-tauri/tauri.conf.json ./tauri.conf.json
COPY --from=web /workspace/dist /workspace/dist
RUN cargo build --release --bin server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libgtk-3-0 libwebkit2gtk-4.1-0 \
    libayatana-appindicator3-1 librsvg2-2 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /workspace/src-tauri/target/release/server /app/server
COPY --from=builder /workspace/dist /app/dist
ENV RUST_LOG=info
ENV KIRO_DATA_DIR=/tmp/kiro-account-manager
EXPOSE 8765
CMD ["/app/server"]
