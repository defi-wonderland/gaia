use actions_indexer_shared::types::ActionRaw;
use actions_indexer_pipeline::processor::{ProcessActions, ActionsProcessor};
use alloy::primitives::{TxHash, Bytes, Address};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BatchSize};
use uuid::Uuid;
use std::sync::Arc;
use actions_indexer::config::handlers::vote::VoteHandler;

/// Creates a single ActionRaw with realistic test data
fn make_raw_action() -> ActionRaw {
    ActionRaw {
        action_type: 0, // Vote action type
        action_version: 1,
        sender: Address::from([0x42; 20]),
        entity: Uuid::new_v4(),
        group_id: Some(Uuid::new_v4()),
        space_pov: Uuid::new_v4(),
        metadata: Some(Bytes::from(vec![0x01, 0x02, 0x03, 0x04])), // Vote payload
        block_number: 12345678,
        block_timestamp: 1672531200, // Jan 1, 2023
        tx_hash: TxHash::from([0x33; 32]),
        object_type: 1,
    }
}

/// Creates a batch of ActionRaw instances for benchmarking
fn make_raw_actions(count: usize) -> Vec<ActionRaw> {
    (0..count)
        .map(|i| ActionRaw {
            action_type: 0,
            action_version: 1,
            sender: Address::from([(i % 256) as u8; 20]),
            entity: Uuid::new_v4(),
            group_id: if i % 2 == 0 { Some(Uuid::new_v4()) } else { None },
            space_pov: Uuid::new_v4(),
            metadata: Some(Bytes::from(vec![0x00])),
            block_number: 12345678 + i as u64,
            block_timestamp: 1672531200 + i as u64,
            tx_hash: TxHash::from([(i % 256) as u8; 32]),
            object_type: 1,
        })
        .collect()
}

/// Benchmark processing a single ActionRaw
fn single_action_processing(c: &mut Criterion) {
    let mut processor = ActionsProcessor::new();
    processor.register_handler(1, 0, 1, Arc::new(VoteHandler));
    
    c.bench_function("process_single_action", |b| {
        b.iter_batched(
            || make_raw_action(),
            |raw_action| {
                let actions = vec![raw_action];
                processor.process(black_box(&actions))
            },
            BatchSize::SmallInput,
        )
    });
}

/// Benchmark processing small batches of ActionRaw (1-10 items)
fn small_batch_processing(c: &mut Criterion) {
    let mut processor = ActionsProcessor::new();
    processor.register_handler(1, 0, 1, Arc::new(VoteHandler));
    
    let mut group = c.benchmark_group("small_batch_processing");
    
    for size in [1, 5, 10].iter() {
        group.bench_with_input(format!("batch_size_{}", size), size, |b, &size| {
            b.iter_batched(
                || make_raw_actions(size),
                |raw_actions| processor.process(black_box(&raw_actions)),
                BatchSize::SmallInput,
            )
        });
    }
    
    group.finish();
}

/// Benchmark processing medium batches of ActionRaw (50-500 items)
fn medium_batch_processing(c: &mut Criterion) {
    let mut processor = ActionsProcessor::new();
    processor.register_handler(1, 0, 1, Arc::new(VoteHandler));
    
    let mut group = c.benchmark_group("medium_batch_processing");
    
    for size in [50, 100, 250, 500].iter() {
        group.bench_with_input(format!("batch_size_{}", size), size, |b, &size| {
            b.iter_batched(
                || make_raw_actions(size),
                |raw_actions| processor.process(black_box(&raw_actions)),
                BatchSize::SmallInput,
            )
        });
    }
    
    group.finish();
}

/// Benchmark processing large batches of ActionRaw (1000+ items)
fn large_batch_processing(c: &mut Criterion) {
    let mut processor = ActionsProcessor::new();
    processor.register_handler(1, 0, 1, Arc::new(VoteHandler));
    
    let mut group = c.benchmark_group("large_batch_processing");
    group.sample_size(10); // Reduce sample size for large batches
    
    for size in [1000, 2500, 5000].iter() {
        group.bench_with_input(format!("batch_size_{}", size), size, |b, &size| {
            b.iter_batched(
                || make_raw_actions(size),
                |raw_actions| processor.process(black_box(&raw_actions)),
                BatchSize::SmallInput,
            )
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    single_action_processing,
    small_batch_processing,
    medium_batch_processing,
    large_batch_processing,
);
criterion_main!(benches);