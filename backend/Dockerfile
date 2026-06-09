FROM rust:1.78-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/velo-backend .
VOLUME /data
EXPOSE 7777
ENV DATA_DIR=/data
CMD ["./velo-backend"]
