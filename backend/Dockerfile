# syntax=docker/dockerfile:1

FROM rust:1.79 AS builder
WORKDIR /workspace
COPY . .
RUN cargo build --release -p openguild-server

FROM debian:bookworm-slim
RUN useradd --uid 1000 openguild && mkdir -p /app
COPY --from=builder /workspace/target/release/openguild-server /app/openguild-server
USER openguild
ENV RUST_LOG=info
EXPOSE 8080
CMD ["/app/openguild-server"]
