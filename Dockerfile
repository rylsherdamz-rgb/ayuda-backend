# --- Stage 1: Build Stage ---
FROM rust:1.81-slim-bookworm AS builder

WORKDIR /app
# Install build dependencies for SSL and Rust compilation
RUN apt-get update && apt-get install -y pkg-config libssl-dev

# Copy source code
COPY . .

# Build the release binary
RUN cargo build --release

# --- Stage 2: Runtime Stage ---
FROM debian:bookworm-slim

WORKDIR /app

# 1. Install necessary libraries for SSL and curl
RUN apt-get update && apt-get install -y \
  curl \
  ca-certificates \
  libssl3 \
  && rm -rf /var/lib/apt/lists/*

# 2. Install Stellar CLI (Pinned to v22.1.0 for stability with Soroban)
# Note: Ensure the architecture matches (x86_64). 
RUN curl -sSfL https://github.com/stellar/stellar-cli/releases/download/v22.1.0/stellar-cli-x86_64-unknown-linux-gnu.tar.gz | tar -xz && \
  mv stellar /usr/local/bin/stellar && \
  chmod +x /usr/local/bin/stellar

# 3. Copy the compiled backend binary from the builder stage
COPY --from=builder /app/target/release/backend /usr/local/bin/ayuda-backend

# 4. Set Environment Variables defaults (Render will override these)
ENV PORT=3000
EXPOSE 3000

# 5. Run the backend
CMD ["ayuda-backend"]
