use hermes_amp::{AmpStreamConfig, stream_actions};
use hermes_instrumentation::info;
use hermes_relay::stream::utils::BlockMetadata as RelayBlockMetadata;
use hermes_relay::Actions;
use prost::Message;

use crate::{Pipeline, PipelineError};

pub async fn run_amp_stream(pipeline: &Pipeline) -> Result<(), PipelineError> {
    let config = AmpStreamConfig::from_env();

    info!(
        flight_url = %config.flight_url,
        dataset = %config.dataset,
        start_block = config.start_block,
        end_block = ?config.end_block,
        actions_address = %config.actions_address,
        "Starting Amp Flight stream source"
    );

    stream_actions(config, move |block| {
        let pipeline_ref = pipeline;
        async move {
            let actions = Actions {
                actions: block.actions,
            };
            let encoded = actions.encode_to_vec();

            let relay_meta = RelayBlockMetadata {
                cursor: block.cursor,
                block_number: block.block_num,
                timestamp: block.timestamp_secs.to_string(),
            };
            let meta = relay_meta.clone().into();

            pipeline_ref
                .process_block_impl(encoded.as_slice(), relay_meta, meta)
                .await
                .map_err(anyhow::Error::from)
        }
    })
    .await
    .map_err(PipelineError::from)
}
