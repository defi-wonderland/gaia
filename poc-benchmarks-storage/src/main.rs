use anyhow::{Context, Result};
use neo4rs::Graph;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::time::{Duration, Instant};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug)]
struct BenchmarkStats {
    mean: Duration,
    median: Duration,
    min: Duration,
    max: Duration,
    std_dev: Duration,
    p50: Duration,
    p90: Duration,
    p95: Duration,
    p99: Duration,
}

impl BenchmarkStats {
    fn from_durations(mut durations: Vec<Duration>) -> Self {
        durations.sort();
        let count = durations.len() as f64;

        let mean = Duration::from_secs_f64(
            durations.iter().map(|d| d.as_secs_f64()).sum::<f64>() / count,
        );

        let median = durations[durations.len() / 2];
        let min = durations[0];
        let max = durations[durations.len() - 1];

        // Calculate standard deviation
        let mean_secs = mean.as_secs_f64();
        let variance = durations
            .iter()
            .map(|d| {
                let diff = d.as_secs_f64() - mean_secs;
                diff * diff
            })
            .sum::<f64>()
            / count;
        let std_dev = Duration::from_secs_f64(variance.sqrt());

        // Calculate percentiles
        let p50 = durations[((count * 0.50) as usize).min(durations.len() - 1)];
        let p90 = durations[((count * 0.90) as usize).min(durations.len() - 1)];
        let p95 = durations[((count * 0.95) as usize).min(durations.len() - 1)];
        let p99 = durations[((count * 0.99) as usize).min(durations.len() - 1)];

        BenchmarkStats {
            mean,
            median,
            min,
            max,
            std_dev,
            p50,
            p90,
            p95,
            p99,
        }
    }

    fn display(&self, name: &str, result_count: usize) {
        info!("{} Results:", name);
        info!("  Results returned: {}", result_count);
        info!("  Mean:     {:.3}ms", self.mean.as_secs_f64() * 1000.0);
        info!("  Median:   {:.3}ms", self.median.as_secs_f64() * 1000.0);
        info!("  Min:      {:.3}ms", self.min.as_secs_f64() * 1000.0);
        info!("  Max:      {:.3}ms", self.max.as_secs_f64() * 1000.0);
        info!("  Std Dev:  {:.3}ms", self.std_dev.as_secs_f64() * 1000.0);
        info!("  P50:      {:.3}ms", self.p50.as_secs_f64() * 1000.0);
        info!("  P90:      {:.3}ms", self.p90.as_secs_f64() * 1000.0);
        info!("  P95:      {:.3}ms", self.p95.as_secs_f64() * 1000.0);
        info!("  P99:      {:.3}ms", self.p99.as_secs_f64() * 1000.0);
    }
}

async fn query_postgres(pool: &PgPool, type_id: &str) -> Result<(Duration, usize)> {
    // Query to get all ranks
    let query = r#"
        SELECT 
            count(distinct fe.string) 
            + count(distinct te.string)
            + count(distinct v.number) as count
        FROM relations r
        LEFT JOIN values fe ON r.from_entity_id = fe.entity_id AND fe.property_id = 'a126ca53-0c8e-48d5-b888-82c734c38935'
        LEFT JOIN values te ON r.to_entity_id = te.entity_id AND te.property_id = 'a126ca53-0c8e-48d5-b888-82c734c38935'
        LEFT JOIN values v ON r.entity_id = v.entity_id AND v.property_id = '665d731a-ee6f-469d-81d2-11da727ca2cf'
        WHERE r.type_id = $1
    "#;

    let start = Instant::now();
    let rows = sqlx::query(query)
        .bind(Uuid::parse_str(type_id)?)
        .fetch_all(pool)
        .await
        .context("Failed to query PostgreSQL")?;

    let time_consumed = start.elapsed();
    let result = rows.first().unwrap().try_get::<i64, _>("count").unwrap() as usize;
    Ok((time_consumed, result))
}

async fn query_neo4j(graph: &Graph, type_id: &str) -> Result<(Duration, usize)> {
    // Optimized query: explicitly specify relationship type and use indexed property
    let query = neo4rs::Query::new(
        "MATCH (m)-[r:RELATES_TO {type_id: $type_id}]->(n) RETURN 
        COUNT(distinct n.prop_a126ca53_0c8e_48d5_b888_82c734c38935_string) 
        + COUNT(distinct m.prop_a126ca53_0c8e_48d5_b888_82c734c38935_string)
        + COUNT(distinct r.prop_665d731a_ee6f_469d_81d2_11da727ca2cf_number) as count".to_string(),
    )
    .param("type_id", type_id);

    let start = Instant::now();
    let mut result = graph.execute(query).await?;
    let time_consumed = start.elapsed();
    let result = result.next().await?.unwrap().get::<usize>("count").unwrap() as usize;
    Ok((time_consumed, result))
}

// Multi-hop traversal queries
async fn query_postgres_multihop(pool: &PgPool, entity_id: &str, min_hops: i32, max_hops: i32) -> Result<(Duration, usize)> {
    // Recursive CTE to find entities within N hops (DIRECTED TRAVERSAL)
    // Only follows edges from from_entity_id -> to_entity_id
    let query = r#"
        WITH RECURSIVE entity_traversal AS (
            -- Base case: start with the given entity
            SELECT 
                $1::uuid AS entity_id,
                0 AS depth
            
            UNION
            
            -- Recursive case: find connected entities through OUTGOING relations only
            SELECT DISTINCT
                r.to_entity_id AS entity_id,
                et.depth + 1 AS depth
            FROM entity_traversal et
            INNER JOIN relations r ON r.from_entity_id = et.entity_id
            WHERE et.depth < $3  -- $3 = maxDepth
        )
        SELECT COUNT(DISTINCT entity_id) AS connected_count
        FROM entity_traversal
        WHERE depth >= $2       -- $2 = minDepth
        AND depth <= $3;      -- $3 = maxDepth
    "#;

    let start = Instant::now();
    let rows = sqlx::query(query)
        .bind(Uuid::parse_str(entity_id)?)
        .bind(min_hops)
        .bind(max_hops)
        .fetch_all(pool)
        .await
        .context("Failed to query PostgreSQL multi-hop")?;

    let time_consumed = start.elapsed();
    let result = rows.first().unwrap().try_get::<i64, _>("connected_count").unwrap() as usize;

    Ok((time_consumed, result))

}

async fn query_neo4j_multihop(graph: &Graph, entity_id: &str, min_hops: i32, max_hops: i32) -> Result<(Duration, usize)> {
    // Variable-length path with DIRECTED relationships (->)
    // Count DISTINCT entities, not all path endpoints (matches PostgreSQL's behavior)
    let query = neo4rs::Query::new(
        format!(
            "MATCH (start:Entity {{id: $entity_id}})-[:RELATES_TO*{}..{}]->(connected:Entity) WITH DISTINCT connected RETURN count(connected) as entity_count",
            min_hops, max_hops
        ),
    )
    .param("entity_id", entity_id);

    let start = Instant::now();
    let mut result = graph.execute(query).await?;

    let time_consumed = start.elapsed();
    let result = result.next().await?.unwrap().get::<usize>("entity_count").unwrap() as usize;
    Ok((time_consumed, result))
}

async fn benchmark_postgres(
    pool: &PgPool,
    type_id: &str,
    iterations: usize,
    warmup: usize,
) -> Result<(BenchmarkStats, usize)> {
    info!("Running PostgreSQL warmup ({} runs)...", warmup);
    for _ in 0..warmup {
        query_postgres(pool, type_id).await?;
    }

    info!("Running PostgreSQL benchmark ({} runs)...", iterations);
    let mut durations = Vec::with_capacity(iterations);
    let mut result_count = 0;
    for i in 0..iterations {
        let (time_consumed, res) = query_postgres(pool, type_id).await?;
        durations.push(time_consumed);
        result_count = res;

        if (i + 1) % 10 == 0 {
            info!("  Progress: {}/{}", i + 1, iterations);
        }
    }

    Ok((BenchmarkStats::from_durations(durations), result_count))
}

async fn benchmark_neo4j(
    graph: &Graph,
    type_id: &str,
    iterations: usize,
    warmup: usize,
) -> Result<(BenchmarkStats, usize)> {
    info!("Running Neo4j warmup ({} runs)...", warmup);
    for _ in 0..warmup {
        query_neo4j(graph, type_id).await?;
    }

    info!("Running Neo4j benchmark ({} runs)...", iterations);
    let mut durations = Vec::with_capacity(iterations);
    let mut result_count = 0;
    for i in 0..iterations {
        let (time_consumed, res) = query_neo4j(graph, type_id).await?;
        durations.push(time_consumed);
        result_count = res;

        if (i + 1) % 10 == 0 {
            info!("  Progress: {}/{}", i + 1, iterations);
        }
    }

    Ok((BenchmarkStats::from_durations(durations), result_count))
}

async fn verify_neo4j_indexes(graph: &Graph) -> Result<()> {
    info!("Checking for required indexes...");
    
    let query = neo4rs::Query::new("SHOW INDEXES".to_string());
    let mut result = graph.execute(query).await.context("Failed to check indexes")?;
    
    let mut index_count = 0;
    let mut has_entity_id = false;
    let mut has_rel_type_id = false;
    
    while let Some(row) = result.next().await? {
        index_count += 1;
        
        // Check for entity.id index
        if let Ok(name) = row.get::<String>("name") {
            if name.contains("entity_id") {
                has_entity_id = true;
            }
            if name.contains("rel_type_id") || name.contains("type_id") {
                has_rel_type_id = true;
            }
        }
    }
    
    info!("Found {} indexes in Neo4j", index_count);
    
    if !has_entity_id {
        tracing::warn!("⚠️  Missing index on Entity.id - performance may be degraded");
        tracing::warn!("   Run: CREATE CONSTRAINT entity_id_unique IF NOT EXISTS FOR (e:Entity) REQUIRE e.id IS UNIQUE");
    } else {
        info!("✓ Entity.id index exists");
    }
    
    if !has_rel_type_id {
        tracing::warn!("⚠️  Missing index on relationship type_id - performance may be degraded");
        tracing::warn!("   Run: CREATE INDEX rel_type_id IF NOT EXISTS FOR ()-[r:RELATES_TO]-() ON (r.type_id)");
    } else {
        info!("✓ Relationship type_id index exists");
    }
    
    if index_count == 0 {
        tracing::warn!("\n⚠️  WARNING: No indexes found! Neo4j performance will be severely degraded.");
        tracing::warn!("Run the migration tool (poc-neo4j-migrate) to create indexes automatically,");
        tracing::warn!("or manually run the script: poc-query-benchmark/setup-neo4j-indexes.cypher\n");
    }
    
    Ok(())
}

async fn get_top_entities_with_relations(_pool: &PgPool) -> Result<Vec<Uuid>> {
    return Ok(vec![Uuid::parse_str("41071c49-2474-6bf3-a851-31ea69b57b20")?]);
}

async fn benchmark_postgres_multihop(
    pool: &PgPool,
    entity_id: &str,
    min_hops: i32,
    max_hops: i32,
    iterations: usize,
    warmup: usize,
) -> Result<(BenchmarkStats, usize)> {
    info!("Running PostgreSQL warmup ({} runs)...", warmup);
    for _ in 0..warmup {
        query_postgres_multihop(pool, entity_id, min_hops, max_hops).await?;
    }

    info!("Running PostgreSQL benchmark ({} runs)...", iterations);
    let mut durations = Vec::with_capacity(iterations);
    let mut result_count = 0;

    for i in 0..iterations {
        let (time_consumed, res) = query_postgres_multihop(pool, entity_id, min_hops, max_hops).await?;
        durations.push(time_consumed);
        result_count = res;

        if (i + 1) % 10 == 0 {
            info!("  Progress: {}/{}", i + 1, iterations);
        }
    }

    Ok((BenchmarkStats::from_durations(durations), result_count))
}

async fn benchmark_neo4j_multihop(
    graph: &Graph,
    entity_id: &str,
    min_hops: i32,
    max_hops: i32,
    iterations: usize,
    warmup: usize,
) -> Result<(BenchmarkStats, usize)> {
    info!("Running Neo4j warmup ({} runs)...", warmup);
    for _ in 0..warmup {
        query_neo4j_multihop(graph, entity_id, min_hops, max_hops).await?;
    }

    info!("Running Neo4j benchmark ({} runs)...", iterations);
    let mut durations = Vec::with_capacity(iterations);
    let mut result_count = 0;

    for i in 0..iterations {
        let (time_consumed, res) = query_neo4j_multihop(graph, entity_id, min_hops, max_hops).await?;
        durations.push(time_consumed);
        result_count = res;

        if (i + 1) % 10 == 0 {
            info!("  Progress: {}/{}", i + 1, iterations);
        }
    }

    Ok((BenchmarkStats::from_durations(durations), result_count))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let neo4j_uri = std::env::var("NEO4J_URI").context("NEO4J_URI must be set")?;
    let iterations: usize = std::env::var("BENCHMARK_ITERATIONS")
        .unwrap_or_else(|_| "100".to_string())
        .parse()
        .context("BENCHMARK_ITERATIONS must be a valid number")?;
    let warmup: usize = std::env::var("BENCHMARK_WARMUP")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .context("BENCHMARK_WARMUP must be a valid number")?;
    
    // Benchmark mode: "direct" or "multihop"
    let benchmark_mode = std::env::var("BENCHMARK_MODE")
        .unwrap_or_else(|_| "both".to_string())
        .to_lowercase();
    
    // Enable query profiling (EXPLAIN/PROFILE) - set PROFILE_QUERIES=true
    let profile_queries = std::env::var("PROFILE_QUERIES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true";

    // Connect to PostgreSQL
    info!("Connecting to PostgreSQL...");
    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;
    info!("✓ Connected to PostgreSQL");

    // Connect to Neo4j
    info!("Connecting to Neo4j...");
    let graph = Graph::new(&neo4j_uri, "", "").context("Failed to connect to Neo4j")?;
    info!("✓ Connected to Neo4j");
    
    // Verify indexes exist for optimal performance
    info!("=== Verifying Neo4j Indexes ===");
    verify_neo4j_indexes(&graph).await?;
    
    if profile_queries {
        info!("⚠️  Query profiling is enabled. Queries will be slower.");
        info!("This is for debugging only. Set PROFILE_QUERIES=false for accurate benchmarks.");
    }
    
    // Run direct query benchmark (ranks lookup)
    if benchmark_mode == "direct" || benchmark_mode == "both" {
        let type_id = "b51f6fc8-556a-40d0-ba87-616a92a626ef";
        
        info!("╔════════════════════════════════════════════════════════════╗");
        info!("║             BENCHMARK 1: GET ALL RANKS RELATIONS           ║");
        info!("╚════════════════════════════════════════════════════════════╝");
        info!("Type ID: {}", type_id);
        info!("Iterations: {}", iterations);
        info!("Warmup runs: {}", warmup);
        info!("");

        // Run PostgreSQL benchmark
        info!("=== PostgreSQL Benchmark ===");
        let (pg_stats, pg_count) = benchmark_postgres(&pg_pool, type_id, iterations, warmup).await?;

        // Run Neo4j benchmark
        info!("");
        info!("=== Neo4j Benchmark ===");
        let (neo4j_stats, neo4j_count) = benchmark_neo4j(&graph, type_id, iterations, warmup).await?;

        // Display results
        info!("╔════════════════════════════════════════════════════════════╗");
        info!("║              BENCHMARK RESULTS COMPARISON                  ║");
        info!("╚════════════════════════════════════════════════════════════╝");

        pg_stats.display("PostgreSQL", pg_count);
        neo4j_stats.display("Neo4j", neo4j_count);

        if pg_count != neo4j_count {
            warn!("Results mismatch: PostgreSQL returned {} results, Neo4j returned {} results", pg_count, neo4j_count);
        }

        // Compare results
        info!("=== Performance Comparison ===");
        info!("");
        let pg_mean_ms = pg_stats.mean.as_secs_f64() * 1000.0;
        let neo4j_mean_ms = neo4j_stats.mean.as_secs_f64() * 1000.0;

        if pg_mean_ms < neo4j_mean_ms {
            let ratio = neo4j_mean_ms / pg_mean_ms;
            info!(
                "🏆 PostgreSQL is FASTER by {:.2}x ({:.3}ms vs {:.3}ms)",
                ratio, pg_mean_ms, neo4j_mean_ms
            );
        } else {
            let ratio = pg_mean_ms / neo4j_mean_ms;
            info!(
                "🏆 Neo4j is FASTER by {:.2}x ({:.3}ms vs {:.3}ms)",
                ratio, neo4j_mean_ms, pg_mean_ms
            );
        }

        info!("");
    }

    // Run multi-hop traversal benchmark
    if benchmark_mode == "multihop" || benchmark_mode == "both" {
        info!("╔════════════════════════════════════════════════════════════╗");
        info!("║         BENCHMARK 2: MULTI-HOP TRAVERSAL                   ║");
        info!("╚════════════════════════════════════════════════════════════╝");
        
        // Get a well-connected entity to start traversal from
        let top_entities = get_top_entities_with_relations(&pg_pool).await?;
        if top_entities.is_empty() {
            info!("⚠️  No entities with relations found. Skipping multi-hop benchmark.");
            return Ok(());
        }
        let sample_entity_id = top_entities[0].to_string();
        let min_hops = 2;
        let max_hops = 4;
        
        info!("Starting entity: {} (well-connected hub)", sample_entity_id);
        info!("Hop range: {}-{}", min_hops, max_hops);
        info!("Iterations: {}", iterations);
        info!("Warmup runs: {}", warmup);
        info!("");

        // Run PostgreSQL benchmark
        info!("=== PostgreSQL Benchmark (Recursive CTE) ===");
        let (pg_stats, pg_count) = benchmark_postgres_multihop(
            &pg_pool, 
            &sample_entity_id, 
            min_hops, 
            max_hops, 
            iterations, 
            warmup
        ).await?;

        // Run Neo4j benchmark
        info!("");
        info!("=== Neo4j Benchmark (Variable-Length Path) ===");
        let (neo4j_stats, neo4j_count) = benchmark_neo4j_multihop(
            &graph,
            &sample_entity_id,
            min_hops,
            max_hops,
            iterations,
            warmup
        ).await?;

        // Display results
        info!("╔════════════════════════════════════════════════════════════╗");
        info!("║              BENCHMARK RESULTS COMPARISON                  ║");
        info!("╚════════════════════════════════════════════════════════════╝");

        pg_stats.display("PostgreSQL (Recursive CTE)", pg_count);
        neo4j_stats.display("Neo4j (Variable-Length Path)", neo4j_count);

        if pg_count != neo4j_count {
            warn!("Results mismatch: PostgreSQL returned {} results, Neo4j returned {} results", pg_count, neo4j_count);
        }

        // Compare results
        info!("=== Performance Comparison ===");
        let pg_mean_ms = pg_stats.mean.as_secs_f64() * 1000.0;
        let neo4j_mean_ms = neo4j_stats.mean.as_secs_f64() * 1000.0;

        info!("PostgreSQL (Recursive CTE): {:.3}ms", pg_mean_ms);
        info!("Neo4j (Variable-Length Path): {:.3}ms", neo4j_mean_ms);
        info!("");

        if pg_mean_ms < neo4j_mean_ms {
            let ratio = neo4j_mean_ms / pg_mean_ms;
            info!(
                "🏆 PostgreSQL is FASTER by {:.2}x ({:.3}ms vs {:.3}ms)",
                ratio, pg_mean_ms, neo4j_mean_ms
            );
        } else {
            let ratio = pg_mean_ms / neo4j_mean_ms;
            info!(
                "🏆 Neo4j (Variable-Length Path) is FASTER by {:.2}x ({:.3}ms vs {:.3}ms)",
                ratio, neo4j_mean_ms, pg_mean_ms
            );
        }

        info!("");
        
    }

    info!("All benchmarks complete!");

    Ok(())
}

