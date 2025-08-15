use std::{collections::HashMap, sync::Arc};

use crate::processor::{HandleAction, ProcessActions};
use actions_indexer_shared::types::{Action, ActionRaw};

/// `ActionsProcessor` is responsible for processing raw `ActionEvent` data into structured `Action` data.
/// It manages a registry of handlers for different action versions and kinds.
pub struct ActionsProcessor {
    handler_registry: HashMap<(u16, u16), Arc<dyn HandleAction>>,
}

impl ActionsProcessor {
    /// Creates a new `ActionsProcessor` instance.
    /// Initializes an empty `handler_registry` for action handlers.
    pub fn new() -> Self {
        Self {
            handler_registry: HashMap::new(),
        }
    }

    /// Registers a handler for a specific action version and kind.
    ///
    /// # Arguments
    ///
    /// * `version` - The version of the action to register the handler for.
    /// * `kind` - The kind of the action to register the handler for.
    /// * `handler` - An `Arc` boxed trait object that implements `HandleAction`,
    ///             responsible for processing the specific action type.
    pub fn register_handler(&mut self, version: u16, kind: u16, handler: Arc<dyn HandleAction>) {
        self.handler_registry.insert((version, kind), handler);
    }
}

impl ProcessActions for ActionsProcessor {
    /// Processes a slice of `ActionRaw`s and returns a vector of `Action`s.
    ///
    /// This method takes an array of raw `ActionRaw`s, applies necessary processing rules,
    /// and converts them into a structured `Action` format.
    ///
    /// # Arguments
    ///
    /// * `actions` - A slice of `ActionRaw`s to be processed.
    ///
    /// # Returns
    ///
    /// A `Vec<Action>` on successful processing.
    fn process(&self, actions: &[ActionRaw]) -> Vec<Action> {
        let mut results = Vec::new();
        for action in actions {
            let handler = self.handler_registry.get(&(action.version, action.kind));
            if let Some(handler) = handler {
                if let Ok(result) = handler.handle(action) {
                    results.push(result);
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::errors::ProcessorError;
    use crate::processor::{ActionsProcessor, HandleAction, ProcessActions};
    use actions_indexer_shared::types::{Action, ActionRaw, Vote, VoteAction};
    use alloy::hex::FromHex;
    use alloy::primitives::{Address, Bytes, TxHash};
    use uuid::uuid;

    struct MockHandler;

    impl HandleAction for MockHandler {
        fn handle(&self, action: &ActionRaw) -> Result<Action, ProcessorError> {
            Ok(Action::Vote(VoteAction {
                raw: action.clone().into(),
                vote: match action.payload[0] {
                    0 => Vote::Up,
                    1 => Vote::Down,
                    2 => Vote::Remove,
                    _ => return Err(ProcessorError::Placeholder),
                },
            }))
        }
    }

    fn make_action_event(payload_byte: u8) -> ActionRaw {
        ActionRaw {
            sender: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
            kind: 1,
            version: 1,
            space_pov: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
            entity: uuid!("a7ef0016-a2f4-44fb-82ca-a4f5c61d2cf5"),
            group_id: uuid!("e50fe85c-108a-4d4a-97b9-376a1e5d318b"),
            payload: Bytes::from(vec![payload_byte]),
            block_number: 1,
            block_timestamp: 1,
            tx_hash: TxHash::from_hex(
                "0x5427daee8d03277f8a30ea881692c04861e692ce5f305b7a689b76248cae63c4",
            )
            .unwrap(),
        }
    }

    fn assert_is_vote_action(action: &Action, event: &ActionRaw, expected_vote: Vote) {
        assert_eq!(
            action,
            &Action::Vote(VoteAction {
                raw: event.clone().into(),
                vote: expected_vote,
            })
        );
    }

    fn mocked_processor() -> ActionsProcessor {
        let mut processor = ActionsProcessor::new();
        processor.register_handler(1, 1, Arc::new(MockHandler));
        processor
    }

    #[test]
    fn test_process_one_up_vote() {
        let processor = mocked_processor();
        let action_event = make_action_event(0);
        let result = processor.process(&[action_event.clone()]);
        assert!(result.len() == 1);
        let action = result[0].clone();
        assert_is_vote_action(&action, &action_event, Vote::Up);
    }

    #[test]
    fn test_process_one_down_vote() {
        let processor = mocked_processor();
        let action_event = make_action_event(1);
        let result = processor.process(&[action_event.clone()]);
        assert!(result.len() == 1);
        let action = result[0].clone();
        assert_is_vote_action(&action, &action_event, Vote::Down);
    }

    #[test]
    fn test_process_one_remove_vote() {
        let processor = mocked_processor();
        let action_event = make_action_event(2);
        let result = processor.process(&[action_event.clone()]);
        assert!(result.len() == 1);
        let action = result[0].clone();
        assert_is_vote_action(&action, &action_event, Vote::Remove);
    }

    #[test]
    fn test_process_multiple_actions() {
        let processor = mocked_processor();
        let action_events = vec![make_action_event(0), make_action_event(1)];
        let result = processor.process(&action_events);
        assert!(result.len() == 2);
        assert_is_vote_action(&result[0], &action_events[0], Vote::Up);
        assert_is_vote_action(&result[1], &action_events[1], Vote::Down);
    }

    #[test]
    fn test_process_invalid_vote() {
        let processor = mocked_processor();
        let action_event = make_action_event(3); // invalid vote
        let result = processor.process(&[action_event.clone()]);
        assert!(result.len() == 0);
    }

    #[test]
    fn test_process_invalid_action_type() {
        let processor = mocked_processor();
        let action_event = ActionRaw {
            sender: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
            kind: 2, // no handler defined for this action type
            version: 1,
            space_pov: Address::from_hex("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap(),
            entity: uuid!("a7ef0016-a2f4-44fb-82ca-a4f5c61d2cf5"),
            group_id: uuid!("e50fe85c-108a-4d4a-97b9-376a1e5d318b"),
            payload: Bytes::from(vec![0]),
            block_number: 1,
            block_timestamp: 1,
            tx_hash: TxHash::from_hex(
                "0x5427daee8d03277f8a30ea881692c04861e692ce5f305b7a689b76248cae63c4",
            )
            .unwrap(),
        };
        let result = processor.process(&[action_event.clone()]);
        assert!(result.len() == 0); // no actions were processed
    }
}
