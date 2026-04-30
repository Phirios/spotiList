FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release --bin spoti-backend

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/spoti-backend /usr/local/bin/spoti-backend

EXPOSE 8080
ENV BIND_ADDR=0.0.0.0:8080
ENTRYPOINT ["/usr/local/bin/spoti-backend"]
