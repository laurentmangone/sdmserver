FROM rust:1.85-slim as builder

WORKDIR /build

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN groupadd -r -g 568 appgroup && useradd -r -u 568 -g appgroup appuser

WORKDIR /app

COPY --from=builder /build/target/release/sdmserver /app/sdmserver
COPY static /app/static

RUN mkdir -p /app/downloads /app/config && chown -R appuser:appgroup /app

# USER appuser

EXPOSE 5900

ENV PORT=5900
ENV DOWNLOAD_DIR=/app/downloads
ENV RUST_LOG=info

CMD ["./sdmserver"]
