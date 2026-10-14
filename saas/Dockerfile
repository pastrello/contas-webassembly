FROM rust:1.90-bookworm AS build
WORKDIR /src
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.105 --locked
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
RUN cargo build --locked --release -p contas-api \
    && cargo build --locked --release -p contas-web --target wasm32-unknown-unknown \
    && wasm-bindgen --target web --out-dir /pkg target/wasm32-unknown-unknown/release/contas_web.wasm
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 app
WORKDIR /app
COPY --from=build /src/target/release/contas-api /app/contas-api
COPY web /app/web
COPY migrations /app/migrations
COPY --from=build /pkg /app/web/pkg
USER app
ENV APP_BIND=0.0.0.0:8080 WEB_DIR=/app/web
EXPOSE 8080
CMD ["/app/contas-api"]
