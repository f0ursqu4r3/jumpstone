# syntax=docker/dockerfile:1

FROM rust:1.82-bookworm AS builder

WORKDIR /workspace

# Install build prerequisites once to keep later layers smaller.
RUN apt-get update \
	&& apt-get install -y --no-install-recommends pkg-config libssl-dev clang \
	&& rm -rf /var/lib/apt/lists/*

# Cache dependency resolution before copying the full source tree.
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml
COPY crates/storage/Cargo.toml crates/storage/Cargo.toml
COPY crates/media/Cargo.toml crates/media/Cargo.toml
COPY crates/crypto/Cargo.toml crates/crypto/Cargo.toml
COPY crates/sfu-client/Cargo.toml crates/sfu-client/Cargo.toml
COPY xtask/Cargo.toml xtask/Cargo.toml

# Create placeholder targets so `cargo fetch` is happy even though the real
# source tree has not been copied into the build context yet.
RUN set -eux; \
	for dir in core crypto media storage sfu-client; do \
		mkdir -p "crates/${dir}/src"; \
		printf "// placeholder for dependency caching\n" > "crates/${dir}/src/lib.rs"; \
	done; \
	for dir in server; do \
		mkdir -p "crates/${dir}/src"; \
		printf "fn main() {}\n" > "crates/${dir}/src/main.rs"; \
	done; \
	mkdir -p xtask/src; \
	printf "fn main() {}\n" > xtask/src/main.rs
RUN cargo fetch --locked

# Bring in the remaining source files and build the release binary.
COPY . .
RUN cargo build --locked --release -p openguild-server

FROM debian:bookworm-slim

RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	&& rm -rf /var/lib/apt/lists/*

RUN useradd --uid 1000 --system --create-home openguild

WORKDIR /app

COPY --from=builder /workspace/target/release/openguild-server /app/openguild-server
COPY --from=builder /workspace/migrations /app/migrations

USER openguild

ENV RUST_LOG=info

EXPOSE 8080 9100

ENTRYPOINT ["/app/openguild-server"]
