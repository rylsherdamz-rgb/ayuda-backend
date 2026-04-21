# --- Stage 1: Build (Compiler Environment) ---
FROM rust:1.81-slim-bookworm AS builder

WORKDIR /app

# Install dependencies required for compiling Rust crypto/SSL crates
RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# Copy your source code
COPY . .

# Build the release binary 
# (This creates /target/release/backend)
RUN cargo build --release

# --- Stage 2: Runtime (Production Environment) ---
FROM debian:bookworm-slim

WORKDIR /app

# 1. Install runtime dependencies for SSL and the installer
RUN apt-get update && apt-get install -y \
  curl \
  ca-certificates \
  libssl3 \
  && rm -rf /var/lib/apt/lists/*

# 2. Install Stellar CLI and Move to Global PATH
# This is the "OS Error 2" fix for Render. 
# It ensures Command::new("stellar") works in your Rust code.
RUN curl -Lsf https://raw.githubusercontent.com/stellar/stellar-cli/main/install.sh | sh && \
  mv /root/.cargo/bin/stellar /usr/local/bin/stellar && \
  chmod +x /usr/local/bin/stellar

# 3. Copy only the compiled binary from the builder stage
COPY --from=builder /app/target/release/backend /usr/local/bin/ayuda-backend

# 4. Networking
# Render provides the $PORT variable automatically.
ENV PORT=10000
EXPOSE 10000

# 5. Start the application
CMD ["ayuda-backend"]
