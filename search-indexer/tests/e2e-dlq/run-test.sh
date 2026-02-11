#!/bin/bash

# E2E test script for the Dead Letter Queue (DLQ) functionality.
# This script generates DLQ-triggering events and validates the results.

set -e

# Default to staging if ENVIRONMENT is not set
export ENVIRONMENT="${ENVIRONMENT:-staging}"

echo "Starting E2E DLQ Test"
echo ""
echo "Environment: $ENVIRONMENT"
echo "This will generate test events to trigger DLQ behavior in the search-indexer"

# Show prefixed topic names based on environment
if [ "$ENVIRONMENT" = "staging" ]; then
    echo "Edits topic: staging.knowledge.edits"
    echo "DLQ topic: staging.search-indexer.dlq"
else
    echo "Edits topic: knowledge.edits"
    echo "DLQ topic: search-indexer.dlq"
fi
echo ""

# Check if Kafka is accessible
if ! timeout 5 bash -c 'cat < /dev/null > /dev/tcp/localhost/9092' 2>/dev/null; then
    echo "Warning: Cannot connect to Kafka at localhost:9092"
    echo "   Make sure Kafka is running:"
    echo "   cd hermes && docker-compose up -d kafka kafka-ui"
    echo ""
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo "Generating DLQ test events..."
echo ""

# Run the event generator using cargo run (builds if needed)
cargo run --release

echo ""
echo "Test events generated successfully!"
echo ""

# Check if OpenSearch is running and run TypeScript validation
if timeout 2 bash -c 'cat < /dev/null > /dev/tcp/localhost/9200' 2>/dev/null; then
    echo "OpenSearch detected at localhost:9200, running validation tests..."
    echo ""

    # Install dependencies if needed
    if [ ! -d "typescript/node_modules" ]; then
        echo "Installing validation script dependencies..."
        cd typescript && npm install --silent && cd ..
        echo ""
    fi

    # Run TypeScript validation
    cd typescript && npm run validate
    VALIDATION_EXIT_CODE=$?
    cd ..
    exit $VALIDATION_EXIT_CODE
else
    echo "OpenSearch not detected at localhost:9200"
    echo ""
    echo "To run the full test, start the required services:"
    echo ""
    echo "1. Start Kafka + OpenSearch:"
    echo "   cd hermes && docker-compose up -d kafka kafka-ui opensearch opensearch-dashboards"
    echo ""
    echo "2. Start the search-indexer with DLQ enabled:"
    echo "   ENVIRONMENT=staging \\"
    echo "   KAFKA_BROKER=localhost:9092 \\"
    echo "   OPENSEARCH_URL=http://localhost:9200 \\"
    echo "   KAFKA_GROUP_EDITS_ID=search-indexer-group-edits-dlq-test-\$(date +%s) \\"
    echo "   KAFKA_GROUP_SCORES_ID=search-indexer-group-scores-dlq-test-\$(date +%s) \\"
    echo "   DLQ_ENABLED=true \\"
    echo "   RUST_LOG=debug,search_indexer=debug \\"
    echo "   cargo run -p search-indexer --features search-indexer-repository/auto_index_creation"
    echo ""
    echo "3. Re-run this test:"
    echo "   ./run-test.sh"
    echo ""
fi

echo "Additional manual checks:"
echo ""
echo "1. View DLQ topic in Kafka UI:"
echo "   http://localhost:8080"
echo ""
echo "2. Read DLQ messages directly:"
if [ "$ENVIRONMENT" = "staging" ]; then
    echo "   kafka-console-consumer --bootstrap-server localhost:9092 --topic staging.search-indexer.dlq --from-beginning"
else
    echo "   kafka-console-consumer --bootstrap-server localhost:9092 --topic search-indexer.dlq --from-beginning"
fi
echo ""
echo "3. Check OpenSearch for indexed entities:"
echo "   curl -s 'http://localhost:9200/entities/_search?pretty' -H 'Content-Type: application/json' -d '{\"query\":{\"match\":{\"name\":\"DLQ Test\"}}}' | jq '.hits.hits[]._source.name'"
echo ""
echo "4. Check search-indexer logs for DLQ/poisoned entity messages:"
echo "   Look for: 'dlq_events', 'poisoned_entities', 'Poisoned entity operation succeeded'"
echo ""
