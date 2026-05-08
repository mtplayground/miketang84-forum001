# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS assets
WORKDIR /app

COPY package.json package-lock.json tailwind.config.js ./
RUN npm ci

COPY assets ./assets
COPY templates ./templates
COPY src ./src

RUN mkdir -p static && npm run build:css

FROM rust:1.90-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
COPY templates ./templates
COPY --from=assets /app/static ./static

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /usr/sbin/nologin --uid 10001 appuser

COPY --from=builder /app/target/release/miketang84-forum001 /usr/local/bin/miketang84-forum001
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static

ENV BIND_ADDR=0.0.0.0:8080
ENV RUST_LOG=info

EXPOSE 8080

USER appuser

CMD ["miketang84-forum001"]
