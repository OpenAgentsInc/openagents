# Autopilot Container Image
# Builds the autopilot-container service for Cloudflare Containers

FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev git openssl-dev pkgconfig

# Set working directory
WORKDIR /app

# Copy entire workspace (needed for workspace deps)
COPY . .

# Build the autopilot-container binary
RUN cargo build --release -p autopilot-container

# Runtime stage - minimal Alpine image
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache \
    git \
    openssh-client \
    ca-certificates

# Create non-root user
RUN adduser -D -u 1000 autopilot
USER autopilot

# Copy the binary
COPY --from=builder /app/target/release/autopilot-container /usr/local/bin/autopilot

# Expose HTTP port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s \
    CMD wget -q --spider http://localhost:8080/ping || exit 1

# Run the service
ENTRYPOINT ["/usr/local/bin/autopilot"]
