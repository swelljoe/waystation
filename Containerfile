FROM rust:1.96-bookworm AS web-builder
RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked --version 0.21.14
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates crates
COPY content content
COPY web web
COPY runtime-assets runtime-assets
RUN trunk build web/index.html --release --dist /dist

FROM rust:1.96-bookworm AS server-builder
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates crates
COPY content content
RUN cargo build --release -p waystation-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=server-builder /src/target/release/waystation-server /usr/local/bin/waystation-server
COPY --from=web-builder /dist /srv/waystation
ENV WAYSTATION_STATIC_DIR=/srv/waystation
ENV WAYSTATION_BIND=0.0.0.0:7777
ENV API_MODE=fixture
EXPOSE 7777
USER 65532:65532
ENTRYPOINT ["waystation-server"]
