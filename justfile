set quiet := true

create-gaia-db:
    docker run --name geo_ranks_poc_postgres \
        -e POSTGRES_PASSWORD=postgres \
        -e POSTGRES_USER=postgres \
        -e POSTGRES_DB=postgres \
        -p 5438:5432 -d postgres:16

    @echo 'Gaia db created: geo_ranks_poc_postgres on port 5438'

load-gaia-dump gaia_dump_file="~/Downloads/gaia_dev_dump.sql":
    cat {{ gaia_dump_file }} | docker exec -i geo_ranks_poc_postgres psql -U postgres -d postgres > /dev/null

    @echo 'Gaia dump loaded'

remove-gaia-db:
    docker rm -f geo_ranks_poc_postgres

    @echo 'Gaia db removed'

create-neo4j-db:
    cd poc-neo4j && docker compose up -d

    @echo 'Starting Neo4j db...'
    sleep 2
    @echo 'Neo4j db created'

run-migration:
    cargo run -p poc-neo4j-migrate

init-ranks-ingesting-server:
    @echo 'Initializing ranks ingesting server...'
    cargo run -p poc-edit-ingest --bin poc-edit-ingest

setup-rank-properties:
    @echo 'Setting up rank properties...'
    cargo run -p poc-edit-ingest --bin setup_rank_properties

ingest-ranks:
    @echo 'Loading ranks from CSV...'
    cargo run -p poc-edit-ingest --bin csv_loader ./poc-edit-ingest/data/random-rankings.csv

run-benchmarks:
    @echo 'Running benchmarks...'
    cargo run -p poc-benchmarks-storage

    @echo 'Benchmarks ran'
