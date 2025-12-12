FROM --platform=linux/amd64 rust:1.85-bookworm as builder

# Install build dependencies for rdkafka
RUN apt-get update && \
    apt-get install -y \
    cmake \
    libclang-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace Cargo.toml and Cargo.lock
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members referenced in Cargo.toml
# (Cargo needs them to resolve the workspace, even if not directly used)
COPY search-indexer ./search-indexer
COPY search-indexer-shared ./search-indexer-shared
COPY search-indexer-repository ./search-indexer-repository
COPY hermes-schema ./hermes-schema
COPY hermes-processor ./hermes-processor
COPY hermes-relay ./hermes-relay
COPY wire ./wire
COPY indexer_utils ./indexer_utils
COPY stream ./stream
COPY indexer ./indexer
COPY cache ./cache
COPY ipfs ./ipfs
COPY sdk ./sdk
COPY atlas ./atlas
COPY mock-substream ./mock-substream
COPY actions-indexer ./actions-indexer
COPY actions-indexer-pipeline ./actions-indexer-pipeline
COPY actions-indexer-shared ./actions-indexer-shared
COPY actions-indexer-repository ./actions-indexer-repository
COPY actions-substream ./actions-substream

# Build the search-indexer binary (native build - no target needed since builder is already Linux)
WORKDIR /app/search-indexer
RUN cargo build --release --package search-indexer

FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    # Create non-root user for running the application
    groupadd -r appuser && \
    useradd -r -g appuser -u 1000 -d /home/appuser -s /sbin/nologin appuser && \
    mkdir -p /home/appuser && \
    chown -R appuser:appuser /home/appuser

COPY --from=builder /app/target/release/search-indexer /usr/local/bin/search-indexer

# Default environment variables
ENV OPENSEARCH_URL=http://opensearch:9200
ENV KAFKA_BROKER=kafka:29092
ENV KAFKA_GROUP_ID=search-indexer
ENV OPENSEARCH_CONNECTION_MODE=retry

# Run as non-root user
USER appuser

CMD ["search-indexer"]

