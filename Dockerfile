# Multi-stage Dockerfile for POC Protection Functions IED
#
# Build:
#   docker build -t poc-ied:latest .
#
# Run:
#   docker run --rm --cap-add SYS_NICE --cap-add NET_RAW --cap-add IPC_LOCK \
#     -e IED_CONFIG=/config/bay1.json \
#     -v $(pwd)/config:/config:ro \
#     poc-ied:latest

# ---------------------------------------------------------------------------
# Stage 1: builder
# ---------------------------------------------------------------------------
FROM rust:1.83-bookworm AS builder

WORKDIR /build

# Cache dependencies before copying source
COPY Cargo.toml Cargo.lock ./
# Create a dummy lib so cargo can fetch dependencies
RUN mkdir -p src && echo "fn main(){}" > src/main.rs && echo "" > src/lib.rs
RUN cargo fetch

# Copy full source and build the release binary
COPY src ./src
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2: runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Minimal runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends libcap2-bin && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy compiled binary
COPY --from=builder /build/target/release/poc_ptoc /app/poc_ptoc

# Copy default configuration directory
COPY config/ /app/config/

# Allow the binary to set SCHED_FIFO without running as root
RUN setcap cap_sys_nice,cap_net_raw,cap_ipc_lock+eip /app/poc_ptoc

ENV IED_CONFIG=/app/config/ied.json

# Use direct exec — no shell wrapper — to avoid extra process overhead
CMD ["/app/poc_ptoc"]
