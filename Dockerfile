# Stage 1: Build the Rust app
FROM rust:latest AS builder

# Install required dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libdbus-1-dev \
    libclang-dev \
    clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release --bin k21-server

# Stage 2: Create runtime image from the same base image
FROM rust:slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    libdbus-1-3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/app/target/release/k21-server /app/

# Set environment variables to ensure server binds to all interfaces
ENV HOST=0.0.0.0

# Expose the port
ENV PORT=8080
EXPOSE 8080

CMD ["./k21-server"]
