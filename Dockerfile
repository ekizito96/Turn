# ─── Stage 1: Build the Turn VM and all providers ────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /build

# Install system deps required by reqwest (OpenSSL)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy Turn VM source
COPY impl/ ./impl/

# Copy all providers
COPY providers/ ./providers/

# Build Turn VM (release)
RUN cargo build --release --manifest-path impl/Cargo.toml

# Build all providers (release)
RUN cargo build --release --workspace --manifest-path providers/Cargo.toml

# ─── Stage 2: Minimal runtime image ──────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install minimal runtime deps (SSL certificates for HTTPS calls from providers)
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy compiled binaries from builder
COPY --from=builder /build/impl/target/release/turn                              /usr/local/bin/turn
COPY --from=builder /build/providers/target/release/turn-provider-openai         /usr/local/bin/turn-provider-openai
COPY --from=builder /build/providers/target/release/turn-provider-azure-openai   /usr/local/bin/turn-provider-azure-openai
COPY --from=builder /build/providers/target/release/turn-provider-azure-anthropic /usr/local/bin/turn-provider-azure-anthropic
COPY --from=builder /build/providers/target/release/turn-provider-aws-anthropic  /usr/local/bin/turn-provider-aws-anthropic

# Create an unprivileged runtime user
RUN useradd -m -u 1001 -s /bin/sh sandbox

# Create a working directory owned by sandbox user
RUN mkdir -p /workspace && chown sandbox:sandbox /workspace

USER sandbox
WORKDIR /workspace

# The entrypoint accepts a Turn script piped to stdin:
#   echo "let x = 1;" | docker run --rm -i turn-sandbox
# Or: docker run --rm -i -e OPENAI_API_KEY=... turn-sandbox < script.tn
ENTRYPOINT ["sh", "-c", "cat > /tmp/script.tn && turn run /tmp/script.tn"]
