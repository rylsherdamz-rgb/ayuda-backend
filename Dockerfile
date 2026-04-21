# --- Stage 1: Build Stage ---
FROM rust:1.81-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies required for SSL/Crypto in Rust
RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# Copy your source code into the container
COPY . .

# Build the release binary based on your [package] name "backend"
RUN cargo build --release

# --- Stage 2: Runtime Stage ---
FROM debian:bookworm-slim

WORKDIR /app

# 1. Install runtime dependencies (SSL and tools for the installer)
RUN apt-get update && apt-get install -y \
  curl \
  ca-certificates \
  libssl3 \
  && rm -rf /var/lib/apt/lists/*

# 2. Install Stellar CLI and move it to a global PATH
# This ensures Command::new("stellar") works in your Rust code
RUN curl -Lsf https://raw.githubusercontent.com/stellar/stellar-cli/main/install.sh | sh && \
  mv /root/.cargo/bin/stellar /usr/local/bin/stellar && \
  chmod +x /usr/local/bin/stellar

# 3. Copy the compiled 'backend' binary from Stage 1
# Note: We rename it to 'ayuda-exe' here just to avoid confusion with folder names
COPY --from=builder /app/target/release/backend /usr/local/bin/ayuda-exe

# 4. Networking configuration
ENV PORT=10000
EXPOSE 10000

# 5. Launch the application
CMD ["ayuda-exe"]
