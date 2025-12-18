# ─────────────────────────────────────────────────────────────────────────────
# Stage 1: Build
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.89-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY payments-types ./payments-types
COPY payments-repo ./payments-repo
COPY payments-hex ./payments-hex
COPY payments-app ./payments-app
COPY payments-client ./payments-client
COPY payments-cli ./payments-cli

# Build release binary with postgres feature (default for production)
RUN cargo build --release -p payments-app --no-default-features --features postgres

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2: Runtime
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/payments-server /app/payments-server

# Create non-root user
RUN useradd -r -s /bin/false payments
USER payments

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run
ENV RUST_LOG=info
CMD ["/app/payments-server"]
