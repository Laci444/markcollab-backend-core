FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
WORKDIR /app

RUN apk add --no-cache \
    build-base \
    cmake \
    coreutils \
    pkgconfig \
    openssl-dev \
    cyrus-sasl-dev \
    zlib-dev \
    zlib-static

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN cargo build --release

FROM alpine:latest AS runtime
WORKDIR /app

RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    openssl \
    cyrus-sasl \
    zlib

RUN addgroup -S appgroup && adduser -S appuser -G appgroup
USER appuser

COPY --from=builder /app/target/release/markcollab-backend-core /usr/local/bin/markcollab-backend-core

EXPOSE 3000

CMD ["markcollab-backend-core"]