---
title: "feat: Governance V2 contract migration"
type: feat
date: 2026-04-01
---

# feat: Governance V2 contract migration

## Overview

Update Gaia's governance indexing pipeline to support the new Geo governance smart contracts before mainnet. This plan builds on the coworker's spec (`2026-03-23-feat-geo-governance-mainnet-indexing-plan.md`) and fills gaps identified during review — primarily the missing `executionGracePeriod` / `executeBy` fields and incomplete proposal-executor detection.

## Context

The Geo governance smart contracts have changed significantly for mainnet. The current indexing pipeline assumes a 4-field VotingSettings struct, a single `threshold` per proposal, and vote payloads without proposal versioning. The new contracts introduce:

- **7-field VotingSettings**: `partialPercentageSupportThreshold`, `universalPercentageSupportThreshold`, `flatSupportThreshold`, `quorum`, `duration`, `disableFastPathAccessForNewMembers`, `executionGracePeriod`
- **8-field ProposalParameters**: adds `executeBy` (absolute timestamp derived from `executionGracePeriod`)
- **Proposal versioning**: votes scoped to `(proposalId, proposalVersion, voteOption)`, version increments on proposal update
- **`VOTING_SETTINGS_UPDATED` action event**: emitted when DAO-global voting settings change
- **Slow-path early execution**: via `universalPercentageSupportThreshold` (percentage of total editors)

If unchanged, the indexer will decode events into outdated shapes, keep stale votes across proposal updates, and compute incorrect proposal status.

### Source documents

- Notion: [[WIP] Smart Contracts V2 UI Impacts](https://www.notion.so/WIP-Smart-Contracts-V2-UI-Impacts-326273e214eb8076a341ce557c122ff6)
- Prior spec: `docs/plans/2026-03-23-feat-geo-governance-mainnet-indexing-plan.md`

### Gaps in prior spec (now addressed)

1. **Missing `executionGracePeriod`**: The `UpdateVotingSettingsAction` and `space_voting_settings` table were missing this field (contract VotingSettings has 7 fields, not 6).
2. **Missing `executeBy` on proposals**: The `ProposalParameters` struct has 8 fields — the `executeBy` absolute timestamp was absent from the proposals table and status logic.
3. **Proposal-executor incomplete**: Detection didn't account for the `executeBy` deadline or new `universalPercentageSupportThreshold` for early execution.
4. **Space archive/recover/clear constants**: Low-effort additions while touching the substream.

---

## Task Breakdown

### Task 1: Add new action constants to hermes-substream and hermes-relay

**Files:**
- `hermes-substream/src/lib.rs`
- `hermes-relay/src/actions.rs`

**Changes:**
- Add `ACTION_VOTING_SETTINGS_UPDATED` constant (keccak256 of the action name used in the contract's `_ping` call)
- Add space lifecycle constants: `ACTION_SPACE_ID_ARCHIVED`, `ACTION_SPACE_ID_RECOVERED`, `ACTION_SPACE_ID_CLEARED` (for future use)
- Re-export new constants in hermes-relay

**Notes:**
- The exact 32-byte hashes must come from the contract source or ABI. These are `keccak256("VOTING_SETTINGS_UPDATED")` etc. — need to verify against contract.
- Space lifecycle constants are low-effort additions while touching these files.

---

### Task 2: Update governance protobuf schema (hermes-schema)

**File:** `hermes-schema/proto/governance.proto`

**Changes:**

1. **Expand `UpdateVotingSettingsAction`** from 4 to 7 fields:
   ```
   partial_percentage_support_threshold (uint64)
   universal_percentage_support_threshold (uint64)
   flat_support_threshold (uint64)
   quorum (uint64)
   duration (uint64)
   disable_fast_path_access_for_new_members (bool)
   execution_grace_period (uint64)
   ```

2. **Replace `ProposalSettings`** with 8-field shape:
   ```
   voting_mode (VotingMode)
   partial_percentage_support_threshold (uint64)
   universal_percentage_support_threshold (uint64)
   flat_support_threshold (uint64)
   quorum (uint64)
   start_date (uint64)
   last_date (uint64)
   execute_by (uint64)
   ```

3. **Add `proposal_version`** (uint32) to `HermesProposalVoted`

4. **Add new message `HermesVotingSettingsUpdated`:**
   ```
   space_id (bytes, 16)
   partial_percentage_support_threshold (uint64)
   universal_percentage_support_threshold (uint64)
   flat_support_threshold (uint64)
   quorum (uint64)
   duration (uint64)
   disable_fast_path_access_for_new_members (bool)
   execution_grace_period (uint64)
   meta (BlockchainMetadata)
   ```

**Constraint:** Do not collapse thresholds back to a single field. Regenerate protobuf bindings after changes.

---

### Task 3: Update hermes-pipeline governance ABI decoding

**File:** `hermes-pipeline/src/decode.rs`

**Changes:**

1. **`VotingSettingsArgs`**: Update from 4-field `(uint256,uint256,uint256,uint256)` to 7-field tuple matching new contract:
   ```
   (uint256, uint256, uint256, uint256, uint256, bool, uint256)
   → partial_percentage_support_threshold, universal_percentage_support_threshold,
     flat_support_threshold, quorum, duration,
     disable_fast_path_access_for_new_members, execution_grace_period
   ```

2. **`ProposalSettingsUsedDataType`**: Update from `(uint256, uint256, uint8, uint256, uint256)` to 8-field:
   ```
   (uint8, uint256, uint256, uint256, uint256, uint256, uint256, uint256)
   → votingMode, partialPercentageSupportThreshold, universalPercentageSupportThreshold,
     flatSupportThreshold, quorum, startDate, lastDate, executeBy
   ```
   Note: exact field order must match contract's `abi.encode(proposal_.parameters)`.

3. **`ProposalVotedDataType`**: Update from `(bytes16, uint8)` to `(bytes16, uint8, uint8)`:
   ```
   → proposalId, proposalVersion, voteOption
   ```

4. **Update `UPDATE_VOTING_SETTINGS` function selector** if the new contract uses a different selector (7-field struct changes the ABI signature). Verify against contract ABI.

---

### Task 4: Update hermes-pipeline governance transform

**File:** `hermes-pipeline/src/pipelines/governance.rs`

**Changes:**

1. Map decoded `ProposalSettingsUsedData` into the new 8-field `ProposalSettings` proto message
2. Include `proposal_version` in `HermesProposalVoted` from the decoded vote payload
3. Add routing for `VOTING_SETTINGS_UPDATED` action:
   - Decode the action event data as the 7-field VotingSettings tuple
   - Emit as `HermesVotingSettingsUpdated` message
4. Update `TransformResult` to include `voting_settings_updated: Vec<HermesVotingSettingsUpdated>`
5. Continue handling orphaned `PROPOSAL_SETTINGS_SELECTED` for fast→slow escalation, emitting the new settings shape

---

### Task 5: Update database schema for governance V2

**File:** `api/src/services/storage/schema.ts`

**Changes to `proposals` table (additive):**
- Add `proposal_version` (integer, not null, default 1)
- Add `partial_percentage_support_threshold` (bigint, nullable — populated for V2 proposals)
- Add `universal_percentage_support_threshold` (bigint, nullable)
- Add `flat_support_threshold` (bigint, nullable)
- Add `execute_by` (bigint, nullable — absolute timestamp for execution deadline)
- Keep existing `threshold`, `quorum` columns for backward compatibility during transition

**Changes to `proposal_actions` table (additive for UpdateVotingSettings):**
- Add `partial_percentage_support_threshold` (bigint, nullable)
- Add `universal_percentage_support_threshold` (bigint, nullable)
- Add `flat_support_threshold` (bigint, nullable)
- Add `disable_fast_path_access_for_new_members` (boolean, nullable)
- Add `execution_grace_period` (bigint, nullable)
- Keep existing `quorum`, `fast_threshold`, `slow_threshold`, `duration` for backward compatibility

**New `space_voting_settings` table:**
```
space_id (uuid, PK, references spaces.id)
partial_percentage_support_threshold (bigint, not null)
universal_percentage_support_threshold (bigint, not null)
flat_support_threshold (bigint, not null)
quorum (bigint, not null)
duration (bigint, not null)
disable_fast_path_access_for_new_members (boolean, not null)
execution_grace_period (bigint, not null)
updated_at (text, not null) — ISO timestamp
updated_at_block (text, not null) — block number
```

**Migration:** Generate via `drizzle-kit generate`. All additive — no column drops.

---

### Task 6: Update KG indexer governance models and handlers

**Files:**
- `kg-indexer/src/models/governance.rs`
- `kg-indexer/src/handlers/governance.rs`

**Model changes:**
- `ProposalItem`: add `proposal_version: i32`, `partial_percentage_support_threshold: i64`, `universal_percentage_support_threshold: i64`, `flat_support_threshold: i64`, `execute_by: Option<i64>`
- `ProposalVoteItem`: add `proposal_version: i32`
- `ProposalActionPayload::UpdateVotingSettings`: expand to 7 fields (add `partial_percentage_support_threshold`, `universal_percentage_support_threshold`, `flat_support_threshold`, `disable_fast_path_access_for_new_members`, `execution_grace_period`)
- Add `SpaceVotingSettingsItem` struct for the new table

**Handler changes:**
- `handle_proposal_created`: map all new proto fields without collapsing thresholds. Set `proposal_version = 1`.
- `handle_proposal_voted`: extract and pass through `proposal_version` from the proto message.
- `handle_proposal_settings_updated` (fast→slow escalation): map new settings fields, preserve `proposal_version`.
- Add `handle_voting_settings_updated`: map `HermesVotingSettingsUpdated` into `SpaceVotingSettingsItem`.

---

### Task 7: Update KG indexer storage for proposal versioning and vote reset

**File:** `kg-indexer/src/storage.rs`

**Changes:**

1. **`insert_proposals()`**: Add new columns to the UNNEST batch insert (proposal_version, partial_percentage_support_threshold, universal_percentage_support_threshold, flat_support_threshold, execute_by). Set `proposal_version = 1` on create.

2. **`update_proposal()`**: Implement atomic version increment:
   - `SET proposal_version = proposals.proposal_version + 1`
   - Use `RETURNING proposal_version` to get the assigned version
   - Delete all `proposal_votes` for the proposal
   - Replace `proposal_actions` for the proposal
   - Reset `yes_count = 0, no_count = 0, abstain_count = 0`

3. **`insert_proposal_votes()`**: Add version validation:
   - Compare event's `proposal_version` to `proposals.proposal_version`
   - Ignore and log votes for non-latest versions
   - Continue to queue tally update only for matching-version votes

4. **`update_proposal_settings()`**: Update for new fields (used by fast→slow escalation)

5. **Add `upsert_space_voting_settings()`**: INSERT ON CONFLICT (space_id) DO UPDATE with all 7 fields + metadata

6. **`process_tally_queue()`**: Update fast-path auto-execution detection to use `flat_support_threshold` instead of generic `threshold`

---

### Task 8: Update KG indexer consumer for VOTING_SETTINGS_UPDATED routing

**File:** `kg-indexer/src/consumer.rs`

**Changes:**
- Add `VotingSettingsUpdated(HermesVotingSettingsUpdated)` variant to `KgMessage` enum
- Add `"VOTING_SETTINGS_UPDATED"` → `KgMessage::VotingSettingsUpdated` route in `parse_message()`
- Route to `handle_voting_settings_updated()` → `storage.upsert_space_voting_settings()` in the main processing loop

---

### Task 9: Update API proposal status computation

**Files:**
- `api/src/proposals/status.ts`
- `api/src/proposals/types.ts`

**Status logic changes:**

1. **Fast path**: Use `flatSupportThreshold` (from new column) instead of generic `threshold`.
   - Formula unchanged: `yesCount >= flatSupportThreshold`
   - Add `executeBy` deadline check: if `nowSeconds > executeBy`, proposal is REJECTED even if threshold met

2. **Slow path late execution** (after voting ends): Use `partialPercentageSupportThreshold`
   - Formula: `(RATIO_BASE - partialPercentageSupportThreshold) * yesCount > partialPercentageSupportThreshold * noCount`
   - Plus quorum check
   - Plus `executeBy` deadline

3. **Slow path early execution** (before voting ends): Use `universalPercentageSupportThreshold`
   - Formula: `yesCount >= ceil(universalPercentageSupportThreshold * totalEditors / RATIO_BASE)`
   - Note: this requires knowing `totalEditors` — may need to be indexed or queried
   - If `totalEditors` is not available in MVP, defer early execution to contract's `canExecuteProposal()` and only implement late execution in SQL

4. **`executeBy` expiration**: Any proposal past its `executeBy` timestamp becomes REJECTED (cannot be executed)

**Type changes:**
- Internal types: add `proposalVersion`, `flatSupportThreshold`, `partialPercentageSupportThreshold`, `universalPercentageSupportThreshold`, `executeBy` to `ProposalWithVotes` / `ProposalListItem`
- Keep `threshold` as a computed compatibility field in the response

---

### Task 10: Update API proposal queries for V2 compatibility

**Files:**
- `api/src/proposals/queries.ts`
- `api/src/proposals/router.ts`

**Query changes:**
- Read new columns from `proposals` table
- Update `sqlIsExecutable()`, `sqlIsProposed()`, `sqlIsRejected()` to use new threshold columns
- Add `executeBy` deadline check to all status fragments
- **Compatibility projection**: keep `threshold` in response as:
  - Fast path: `threshold = flat_support_threshold`
  - Slow path: `threshold = partial_percentage_support_threshold`
- Add `proposal_actions` JSON aggregation for new UpdateVotingSettings fields

**Router changes:**
- Add additive metadata fields: `proposalVersion`, `executeBy`
- Extend `UpdateVotingSettingsAction` response with new optional fields
- Keep all existing response field names stable

---

### Task 11: Update proposal-executor detection for V2 semantics

**File:** `proposal-executor/src/detect.ts`

**Changes:**
- Replace `p.threshold` with `p.partial_percentage_support_threshold` in the slow-path formula
- Add `executeBy` deadline: `AND $1::bigint <= p.execute_by` (must be within execution window)
- Replace `MAX_PROPOSAL_AGE` heuristic with `execute_by` column when available (fall back to MAX_PROPOSAL_AGE for old proposals without execute_by)
- Consider adding slow-path early execution detection using `universal_percentage_support_threshold` if `totalEditors` is available

---

## Dependency Order

```
Task 1 (substream constants)
  ↓
Task 2 (proto schema) → regenerate bindings
  ↓
Task 3 (pipeline decode) + Task 4 (pipeline transform) [parallel, same service]
  ↓
Task 5 (DB schema) → generate migration
  ↓
Task 6 (KG models/handlers) + Task 7 (KG storage) + Task 8 (KG consumer) [parallel, same service]
  ↓
Task 9 (API status) + Task 10 (API queries) [parallel, same service]
  ↓
Task 11 (proposal-executor)
```

## Rollout

- Deploy Hermes (tasks 1–4) first — new proto fields are additive
- Deploy DB migration (task 5) before KG indexer
- Deploy KG indexer (tasks 6–8) — starts writing V2 fields
- Deploy API + executor (tasks 9–11) last — reads V2 fields

## Open Questions

1. **`totalEditors` for slow-path early execution**: The `universalPercentageSupportThreshold` formula requires knowing the total number of editors in the DAO. Is this currently indexed? If not, do we defer early execution to the contract's `canExecuteProposal()` view function?

2. **Action hash verification**: The exact keccak256 hashes for `VOTING_SETTINGS_UPDATED`, `SPACE_ID_ARCHIVED`, `SPACE_ID_RECOVERED`, `SPACE_ID_CLEARED` need to be confirmed from the contract source.

3. **ABI field ordering**: The exact Solidity `abi.encode` order for `ProposalParameters` and the new `VotingSettings` must be verified against the contract source. The plan assumes the struct field order matches the encode order.

4. **`updateVotingSettings` function selector**: If the function signature changed (7-field struct vs 4-field), the 4-byte selector `[0xd2, 0x1e, 0x85, 0x41]` will be different. Needs verification.

## Verification

- **Unit tests**: Update existing proposal status tests in API for new threshold logic and `executeBy` deadline
- **KG indexer tests**: Test proposal create with V2 fields, proposal update with version increment + vote reset, vote rejection for non-latest version
- **Integration**: Index test governance events through full pipeline and verify DB state matches expected V2 shape
- **Compatibility**: Verify existing API response contracts remain stable (threshold, quorum, status, canExecute fields preserved)
