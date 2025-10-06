pub mod helpers;

mod pb;

use member_access_plugin::events::AddMemberProposalCreated as AddMemberProposalCreatedEvent;
use pb::schema::{
    AddEditorProposalCreated, AddEditorProposalsCreated, AddMemberProposalCreated,
    AddMemberProposalsCreated, AddSubspaceProposalCreated, AddSubspaceProposalsCreated,
    EditPublished, EditorAdded, EditorRemoved, EditorsAdded, EditorsRemoved, EditsPublished,
    GeoGovernancePluginCreated, GeoGovernancePluginsCreated, GeoOutput,
    GeoPersonalSpaceAdminPluginCreated, GeoPersonalSpaceAdminPluginsCreated, GeoSpaceCreated,
    GeoSpacesCreated, MemberAdded, MemberRemoved,
    MembersAdded, MembersRemoved, ProposalExecuted, ProposalsExecuted, PublishEditProposalCreated,
    PublishEditsProposalsCreated, RemoveEditorProposalCreated, RemoveEditorProposalsCreated,
    RemoveMemberProposalCreated, RemoveMemberProposalsCreated, RemoveSubspaceProposalCreated,
    RemoveSubspaceProposalsCreated, SubspaceAdded, SubspaceRemoved, SubspacesAdded,
    SubspacesRemoved, SuccessorSpaceCreated, SuccessorSpacesCreated, VoteCast, VotesCast,
};

use substreams_ethereum::{pb::eth, use_contract, Event};

use helpers::*;

use_contract!(space, "abis/space.json");
use_contract!(space_setup, "abis/space-setup.json");
use_contract!(governance_setup, "abis/governance-setup.json");
use_contract!(personal_admin_setup, "abis/personal-admin-setup.json");
use_contract!(personal_admin_plugin, "abis/personal-admin-plugin.json");
use_contract!(main_voting_plugin, "abis/main-voting-plugin.json");
use_contract!(member_access_plugin, "abis/member-access-plugin.json");
use_contract!(
    majority_voting_base_plugin,
    "abis/majority-voting-base.json"
);

use governance_setup::events::GeoGovernancePluginsCreated as GovernancePluginCreatedEvent;
use main_voting_plugin::events::{
    AcceptSubspaceProposalCreated as AcceptSubspaceProposalCreatedEvent,
    AddEditorProposalCreated as AddEditorProposalCreatedEvent, EditorAdded as EditorAddedEvent,
    EditorRemoved as EditorRemovedEvent, EditorsAdded as EditorsAddedEvent,
    MemberAdded as MemberAddedEvent, MemberRemoved as MemberRemovedEvent,
    MembersAdded as MembersAddedEvent, ProposalExecuted as ProposalExecutedEvent,
    PublishEditsProposalCreated as PublishEditsProposalCreatedEvent,
    RemoveEditorProposalCreated as RemoveEditorProposalCreatedEvent,
    RemoveMemberProposalCreated as RemoveMemberProposalCreatedEvent,
    RemoveSubspaceProposalCreated as RemoveSubspaceProposalCreatedEvent,
};
use majority_voting_base_plugin::events::VoteCast as VoteCastEvent;
use personal_admin_setup::events::GeoPersonalAdminPluginCreated as GeoPersonalAdminPluginCreatedEvent;
use space::events::{
    EditsPublished as EditsPublishedEvent, SubspaceAccepted as SubspaceAcceptedEvent,
    SubspaceRemoved as SubspaceRemovedEvent, SuccessorSpaceCreated as SuccessorSpaceCreatedEvent,
};
use space_setup::events::GeoSpacePluginCreated as SpacePluginCreatedEvent;

/**
 * The new DAO-based contracts allow forking of spaces into successor spaces. This is so
 * users can create new spaces whose data is derived from another space.
 *
 * This is immediately useful when migrating from legacy spaces to the new DAO-based spaces,
 * but it's generally applicable across any space.
 */
#[substreams::handlers::map]
fn map_successor_spaces_created(
    block: eth::v2::Block,
) -> Result<SuccessorSpacesCreated, substreams::errors::Error> {
    let successor_spaces: Vec<SuccessorSpaceCreated> = block
        .logs()
        .filter_map(|log| {
            let address = format_hex(&log.address());

            if let Some(successor_space_created) = SuccessorSpaceCreatedEvent::match_and_decode(log)
            {
                return Some(SuccessorSpaceCreated {
                    plugin_address: address,
                    predecessor_space: format_hex(&successor_space_created.predecessor_space),
                    dao_address: format_hex(&successor_space_created.dao),
                });
            }

            return None;
        })
        .collect();

    Ok(SuccessorSpacesCreated {
        spaces: successor_spaces,
    })
}

/**
 * The new DAO-based space contracts are based on Aragon's OSX architecture which uses
 * plugins to define functionality assigned to a DAO (See the top level comment for more
 * information on Aragon's DAO architecture).
 *
 * This handler maps creation of the Space plugin and associates the Space plugin contract
 * address with the address of the DAO contract.
 */
#[substreams::handlers::map]
fn map_spaces_created(
    block: eth::v2::Block,
) -> Result<GeoSpacesCreated, substreams::errors::Error> {
    let spaces: Vec<GeoSpaceCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(space_created) = SpacePluginCreatedEvent::match_and_decode(log) {
                return Some(GeoSpaceCreated {
                    dao_address: format_hex(&space_created.dao),
                    space_address: format_hex(&space_created.plugin),
                });
            }

            return None;
        })
        .collect();

    Ok(GeoSpacesCreated { spaces })
}

#[substreams::handlers::map]
fn map_subspaces_added(block: eth::v2::Block) -> Result<SubspacesAdded, substreams::errors::Error> {
    let subspaces: Vec<SubspaceAdded> = block
        .logs()
        .filter_map(|log| {
            if let Some(space_created) = SubspaceAcceptedEvent::match_and_decode(log) {
                return Some(SubspaceAdded {
                    change_type: "added".to_string(),
                    subspace: format_hex(&space_created.subspace_dao),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&space_created.dao),
                });
            }

            return None;
        })
        .collect();

    Ok(SubspacesAdded { subspaces })
}

#[substreams::handlers::map]
fn map_subspaces_removed(
    block: eth::v2::Block,
) -> Result<SubspacesRemoved, substreams::errors::Error> {
    let subspaces: Vec<SubspaceRemoved> = block
        .logs()
        .filter_map(|log| {
            if let Some(space_created) = SubspaceRemovedEvent::match_and_decode(log) {
                return Some(SubspaceRemoved {
                    change_type: "removed".to_string(),
                    subspace: format_hex(&space_created.subspace_dao),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&space_created.dao),
                });
            }

            return None;
        })
        .collect();

    Ok(SubspacesRemoved { subspaces })
}

/**
 * The new DAO-based space contracts are based on Aragon's OSX architecture which uses
 * plugins to define functionality assigned to a DAO (See the top level comment for more
 * information on Aragon's DAO architecture).
 *
 * This handler maps creation of any governance plugins and associates the governance plugins
 * contract addresses with the address of the DAO contract.
 *
 * As of January 23, 2024 there are two governance plugins:
 * 1. Voting plugin – This defines the voting and proposal rules and behaviors for a DAO
 * 2. Member access plugin – This defines the membership rules and behaviors for a DAO
 */
#[substreams::handlers::map]
fn map_governance_plugins_created(
    block: eth::v2::Block,
) -> Result<GeoGovernancePluginsCreated, substreams::errors::Error> {
    let plugins: Vec<GeoGovernancePluginCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(space_governance_created) =
                GovernancePluginCreatedEvent::match_and_decode(log)
            {
                return Some(GeoGovernancePluginCreated {
                    dao_address: format_hex(&space_governance_created.dao),
                    main_voting_address: format_hex(&space_governance_created.main_voting_plugin),
                    member_access_address: format_hex(
                        &space_governance_created.member_access_plugin,
                    ),
                });
            }

            return None;
        })
        .collect();

    Ok(GeoGovernancePluginsCreated { plugins })
}

#[substreams::handlers::map]
fn map_personal_admin_plugins_created(
    block: eth::v2::Block,
) -> Result<GeoPersonalSpaceAdminPluginsCreated, substreams::errors::Error> {
    let plugins: Vec<GeoPersonalSpaceAdminPluginCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(personal_space_created) =
                GeoPersonalAdminPluginCreatedEvent::match_and_decode(log)
            {
                return Some(GeoPersonalSpaceAdminPluginCreated {
                    dao_address: format_hex(&personal_space_created.dao),
                    personal_admin_address: (format_hex(
                        &personal_space_created.personal_admin_plugin,
                    )),
                });
            }

            return None;
        })
        .collect();

    Ok(GeoPersonalSpaceAdminPluginsCreated { plugins })
}

#[substreams::handlers::map]
fn map_members_added(block: eth::v2::Block) -> Result<MembersAdded, substreams::errors::Error> {
    _map_members_added(block)
}

fn _map_members_added(block: eth::v2::Block) -> Result<MembersAdded, substreams::errors::Error> {
    let members: Vec<MemberAdded> = block
        .logs()
        .flat_map(|log| {
            // Handle individual MemberAdded events
            if let Some(member_added) = MemberAddedEvent::match_and_decode(log) {
                return vec![MemberAdded {
                    change_type: "added".to_string(),
                    main_voting_plugin_address: format_hex(&log.address()),
                    member_address: format_hex(&member_added.member),
                    dao_address: format_hex(&member_added.dao),
                }];
            }
            // Handle batch MembersAdded events
            else if let Some(members_added) = MembersAddedEvent::match_and_decode(log) {
                return members_added
                    .members
                    .into_iter()
                    .map(|member| MemberAdded {
                        change_type: "added".to_string(),
                        main_voting_plugin_address: format_hex(&log.address()),
                        member_address: format_hex(&member),
                        dao_address: format_hex(&members_added.dao),
                    })
                    .collect();
            }

            vec![]
        })
        .collect();

    Ok(MembersAdded { members })
}

#[substreams::handlers::map]
fn map_members_removed(block: eth::v2::Block) -> Result<MembersRemoved, substreams::errors::Error> {
    let members: Vec<MemberRemoved> = block
        .logs()
        .filter_map(|log| {
            if let Some(members_approved) = MemberRemovedEvent::match_and_decode(log) {
                return Some(MemberRemoved {
                    change_type: "removed".to_string(),
                    dao_address: format_hex(&members_approved.dao),
                    plugin_address: format_hex(&log.address()),
                    member_address: format_hex(&members_approved.member),
                });
            }

            return None;
        })
        .collect();

    Ok(MembersRemoved { members })
}

#[substreams::handlers::map]
fn map_editors_added(block: eth::v2::Block) -> Result<EditorsAdded, substreams::errors::Error> {
    _map_editors_added(block)
}

fn _map_editors_added(block: eth::v2::Block) -> Result<EditorsAdded, substreams::errors::Error> {
    let editors: Vec<EditorAdded> = block
        .logs()
        .flat_map(|log| {
            // Handle individual EditorAdded events
            if let Some(editor_added) = EditorAddedEvent::match_and_decode(log) {
                return vec![EditorAdded {
                    change_type: "added".to_string(),
                    main_voting_plugin_address: format_hex(&log.address()),
                    editor_address: format_hex(&editor_added.editor),
                    dao_address: format_hex(&editor_added.dao),
                }];
            }
            // Handle batch EditorsAdded events
            else if let Some(editors_added) = EditorsAddedEvent::match_and_decode(log) {
                return editors_added
                    .editors
                    .into_iter()
                    .map(|editor| EditorAdded {
                        change_type: "added".to_string(),
                        main_voting_plugin_address: format_hex(&log.address()),
                        editor_address: format_hex(&editor),
                        dao_address: format_hex(&editors_added.dao),
                    })
                    .collect();
            }

            vec![]
        })
        .collect();

    Ok(EditorsAdded { editors })
}

#[substreams::handlers::map]
fn map_editors_removed(block: eth::v2::Block) -> Result<EditorsRemoved, substreams::errors::Error> {
    let editors: Vec<EditorRemoved> = block
        .logs()
        .filter_map(|log| {
            if let Some(members_approved) = EditorRemovedEvent::match_and_decode(log) {
                return Some(EditorRemoved {
                    change_type: "removed".to_string(),
                    plugin_address: format_hex(&log.address()),
                    editor_address: format_hex(&members_approved.editor),
                    dao_address: format_hex(&members_approved.dao),
                });
            }

            return None;
        })
        .collect();

    Ok(EditorsRemoved { editors })
}

/**
* Proposals for each proposal type
* 1. Add member
* 2. Remove member
* 3. Add editor
* 4. Remove editor
* 5. Add subspace
* 6. Remove subspace
* 7. Publish edits
*/

/**
 * Proposals represent a proposal to change the state of a DAO-based space. Proposals can
 * represent changes to content, membership (editor or member), governance changes, subspace
 * membership, or anything else that can be executed by a DAO.
 *
 * Currently we use a simple majority voting model, where a proposal requires 51% of the
 * available votes in order to pass. Only editors are allowed to vote on proposals, but editors
 * _and_ members can create them.
 *
 * Proposals require encoding a "callback" that represents the action to be taken if the proposal
 * succeeds. For example, if a proposal is to add a new editor to the space, the callback would
 * be the encoded function call to add the editor to the space.
 *
 * ```ts
 * {
 *   to: `0x123...`, // The address of the membership contract
 *   data: `0x123...`, // The encoded function call parameters
 * }
 * ```
 */
// #[substreams::handlers::map]
// fn map_proposals_created(
//     block: eth::v2::Block,
// ) -> Result<ProposalsCreated, substreams::errors::Error> {
//     let proposals: Vec<ProposalCreated> = block
//         .logs()
//         .filter_map(|log| {
//             if let Some(proposal_created) = ProposalCreatedEvent::match_and_decode(log) {
//                 // @TODO: Should we return none if actions is empty?
//                 return Some(ProposalCreated {
//                     proposal_id: proposal_created.proposal_id.to_string(),
//                     creator: format_hex(&proposal_created.creator),
//                     start_time: proposal_created.start_date.to_string(),
//                     end_time: proposal_created.end_date.to_string(),
//                     metadata_uri: String::from_utf8(proposal_created.metadata).unwrap(),
//                     plugin_address: format_hex(&log.address()),
//                 });
//             }

//             return None;
//         })
//         .collect();

//     Ok(ProposalsCreated { proposals })
// }

#[substreams::handlers::map]
fn map_proposals_executed(
    block: eth::v2::Block,
) -> Result<ProposalsExecuted, substreams::errors::Error> {
    let executed_proposals: Vec<ProposalExecuted> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposal_created) = ProposalExecutedEvent::match_and_decode(log) {
                return Some(ProposalExecuted {
                    plugin_address: format_hex(&log.address()),
                    proposal_id: proposal_created.proposal_id.to_string(),
                });
            }

            return None;
        })
        .collect();

    Ok(ProposalsExecuted { executed_proposals })
}

/**
 * Processed Proposals represent content that has been approved by a DAO
 * and executed onchain.
 *
 * We use the content URI to represent the content that was approved. We
 * only consume the `proposalId` in the content URI to map the processed
 * data to an existing proposal onchain and in the sink.
*/
#[substreams::handlers::map]
fn map_edits_published(block: eth::v2::Block) -> Result<EditsPublished, substreams::errors::Error> {
    let edits: Vec<EditPublished> = block
        .logs()
        .filter_map(|log| {
            if let Some(edit_published) = EditsPublishedEvent::match_and_decode(log) {
                return Some(EditPublished {
                    content_uri: edit_published.edits_content_uri,
                    dao_address: format_hex(&edit_published.dao),
                    plugin_address: format_hex(&log.address()),
                });
            }

            return None;
        })
        .collect();

    Ok(EditsPublished { edits })
}

/**
 * Votes represent a vote on a proposal in a DAO-based space.
 *
 * Currently we use a simple majority voting model, where a proposal requires 51% of the
 * available votes in order to pass. Only editors are allowed to vote on proposals, but editors
 * _and_ members can create them.
 */
#[substreams::handlers::map]
fn map_votes_cast(block: eth::v2::Block) -> Result<VotesCast, substreams::errors::Error> {
    let votes: Vec<VoteCast> = block
        .logs()
        .filter_map(|log| {
            // @TODO: Should we track our plugins/daos and only emit if the address is one of them?
            if let Some(vote_cast) = VoteCastEvent::match_and_decode(log) {
                return Some(VoteCast {
                    // The onchain proposal id is an incrementing integer. We represent
                    // the proposal with a more unique id in the sink, so we remap the
                    // name here to disambiguate between the onchain id and the sink id.
                    onchain_proposal_id: vote_cast.proposal_id.to_string(),
                    voter: format_hex(&vote_cast.voter),
                    plugin_address: format_hex(&log.address()),
                    vote_option: vote_cast.vote_option.to_u64(),
                });
            }

            return None;
        })
        .collect();

    Ok(VotesCast { votes })
}

#[substreams::handlers::map]
fn map_publish_edits_proposals_created(
    block: eth::v2::Block,
) -> Result<PublishEditsProposalsCreated, substreams::errors::Error> {
    let edits: Vec<PublishEditProposalCreated> = block
        .logs()
        .filter_map(|log| {
            // @TODO: Should we track our plugins/daos and only emit if the address is one of them?
            if let Some(proposed_edit) = PublishEditsProposalCreatedEvent::match_and_decode(log) {
                return Some(PublishEditProposalCreated {
                    // The onchain proposal id is an incrementing integer. We represent
                    // the proposal with a more unique id in the sink, so we remap the
                    // name here to disambiguate between the onchain id and the sink id.
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    content_uri: proposed_edit.edits_content_uri,
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                });
            }

            return None;
        })
        .collect();

    Ok(PublishEditsProposalsCreated { edits })
}

#[substreams::handlers::map]
fn map_add_member_proposals_created(
    block: eth::v2::Block,
) -> Result<AddMemberProposalsCreated, substreams::errors::Error> {
    let proposed_members: Vec<AddMemberProposalCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposed_edit) = AddMemberProposalCreatedEvent::match_and_decode(log) {
                return Some(AddMemberProposalCreated {
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                    change_type: "added".to_string(),
                    member: format_hex(&proposed_edit.member),
                });
            }

            return None;
        })
        .collect();

    Ok(AddMemberProposalsCreated { proposed_members })
}

#[substreams::handlers::map]
fn map_remove_member_proposals_created(
    block: eth::v2::Block,
) -> Result<RemoveMemberProposalsCreated, substreams::errors::Error> {
    let proposed_members: Vec<RemoveMemberProposalCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposed_edit) = RemoveMemberProposalCreatedEvent::match_and_decode(log) {
                return Some(RemoveMemberProposalCreated {
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                    change_type: "removed".to_string(),
                    member: format_hex(&proposed_edit.member),
                });
            }

            return None;
        })
        .collect();

    Ok(RemoveMemberProposalsCreated { proposed_members })
}

#[substreams::handlers::map]
fn map_add_editor_proposals_created(
    block: eth::v2::Block,
) -> Result<AddEditorProposalsCreated, substreams::errors::Error> {
    let proposed_editors: Vec<AddEditorProposalCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposed_edit) = AddEditorProposalCreatedEvent::match_and_decode(log) {
                return Some(AddEditorProposalCreated {
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                    change_type: "added".to_string(),
                    editor: format_hex(&proposed_edit.editor),
                });
            }

            return None;
        })
        .collect();

    Ok(AddEditorProposalsCreated { proposed_editors })
}

#[substreams::handlers::map]
fn map_remove_editor_proposals_created(
    block: eth::v2::Block,
) -> Result<RemoveEditorProposalsCreated, substreams::errors::Error> {
    let proposed_editors: Vec<RemoveEditorProposalCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposed_edit) = RemoveEditorProposalCreatedEvent::match_and_decode(log) {
                return Some(RemoveEditorProposalCreated {
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                    change_type: "removed".to_string(),
                    editor: format_hex(&proposed_edit.editor),
                });
            }

            return None;
        })
        .collect();

    Ok(RemoveEditorProposalsCreated { proposed_editors })
}

#[substreams::handlers::map]
fn map_add_subspace_proposals_created(
    block: eth::v2::Block,
) -> Result<AddSubspaceProposalsCreated, substreams::errors::Error> {
    let proposed_subspaces: Vec<AddSubspaceProposalCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposed_edit) = AcceptSubspaceProposalCreatedEvent::match_and_decode(log) {
                return Some(AddSubspaceProposalCreated {
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                    change_type: "added".to_string(),
                    subspace: format_hex(&proposed_edit.subspace),
                });
            }

            return None;
        })
        .collect();

    Ok(AddSubspaceProposalsCreated { proposed_subspaces })
}

#[substreams::handlers::map]
fn map_remove_subspace_proposals_created(
    block: eth::v2::Block,
) -> Result<RemoveSubspaceProposalsCreated, substreams::errors::Error> {
    let proposed_subspaces: Vec<RemoveSubspaceProposalCreated> = block
        .logs()
        .filter_map(|log| {
            if let Some(proposed_edit) = RemoveSubspaceProposalCreatedEvent::match_and_decode(log) {
                return Some(RemoveSubspaceProposalCreated {
                    proposal_id: proposed_edit.proposal_id.to_string(),
                    creator: format_hex(&proposed_edit.creator),
                    start_time: proposed_edit.start_date.to_string(),
                    end_time: proposed_edit.end_date.to_string(),
                    plugin_address: format_hex(&log.address()),
                    dao_address: format_hex(&proposed_edit.dao),
                    change_type: "added".to_string(),
                    subspace: format_hex(&proposed_edit.subspace),
                });
            }

            return None;
        })
        .collect();

    Ok(RemoveSubspaceProposalsCreated { proposed_subspaces })
}

#[substreams::handlers::map]
fn geo_out(
    spaces_created: GeoSpacesCreated,
    governance_plugins_created: GeoGovernancePluginsCreated,
    votes_cast: VotesCast,
    edits_published: EditsPublished,
    successor_spaces_created: SuccessorSpacesCreated,
    subspaces_added: SubspacesAdded,
    subspaces_removed: SubspacesRemoved,
    proposals_executed: ProposalsExecuted,
    members_added: MembersAdded,
    editors_added: EditorsAdded,
    personal_admin_plugins_created: GeoPersonalSpaceAdminPluginsCreated,
    members_removed: MembersRemoved,
    editors_removed: EditorsRemoved,
    edit_proposals: PublishEditsProposalsCreated,
    proposed_added_members: AddMemberProposalsCreated,
    proposed_removed_members: RemoveMemberProposalsCreated,
    proposed_added_editors: AddEditorProposalsCreated,
    proposed_removed_editors: RemoveEditorProposalsCreated,
    proposed_added_subspaces: AddSubspaceProposalsCreated,
    proposed_removed_subspaces: RemoveSubspaceProposalsCreated,
) -> Result<GeoOutput, substreams::errors::Error> {
    let spaces_created = spaces_created.spaces;
    let governance_plugins_created = governance_plugins_created.plugins;
    let votes_cast = votes_cast.votes;
    let edits_published = edits_published.edits;
    let successor_spaces_created = successor_spaces_created.spaces;
    let added_subspaces = subspaces_added.subspaces;
    let removed_subspaces = subspaces_removed.subspaces;
    let executed_proposals = proposals_executed.executed_proposals;
    let members_added = members_added.members;
    let editors_added = editors_added.editors;
    let members_removed = members_removed.members;
    let editors_removed = editors_removed.editors;
    let personal_admin_plugins_created = personal_admin_plugins_created.plugins;
    let edit_proposals_created = edit_proposals.edits;

    Ok(GeoOutput {
        spaces_created,
        governance_plugins_created,
        votes_cast,
        edits_published,
        successor_spaces_created,
        subspaces_added: added_subspaces,
        subspaces_removed: removed_subspaces,
        executed_proposals,
        members_added,
        editors_added,
        personal_plugins_created: personal_admin_plugins_created,
        members_removed,
        editors_removed,
        edits: edit_proposals_created,
        proposed_added_members: proposed_added_members.proposed_members,
        proposed_removed_members: proposed_removed_members.proposed_members,
        proposed_added_editors: proposed_added_editors.proposed_editors,
        proposed_removed_editors: proposed_removed_editors.proposed_editors,
        proposed_added_subspaces: proposed_added_subspaces.proposed_subspaces,
        proposed_removed_subspaces: proposed_removed_subspaces.proposed_subspaces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use substreams_ethereum::pb::eth::v2::{Block, BlockHeader, TransactionTrace, TransactionReceipt, Log};
    use prost_types::Timestamp;

    /// Helper function to create a mock Block with given logs
    fn create_mock_block_with_logs(logs: Vec<Log>) -> Block {
        let block_header = BlockHeader {
            timestamp: Some(Timestamp {
                seconds: 1650000000,
                nanos: 0,
            }),
            number: 12345,
            ..Default::default()
        };

        let transaction_receipt = TransactionReceipt {
            logs,
            ..Default::default()
        };

        let transaction_trace = TransactionTrace {
            hash: vec![0x12; 32],
            from: vec![0xab; 20],
            receipt: Some(transaction_receipt),
            status: 1,
            ..Default::default()
        };

        Block {
            number: 12345,
            hash: vec![0x12; 32],
            header: Some(block_header),
            transaction_traces: vec![transaction_trace],
            ..Default::default()
        }
    }

    /// Creates a properly ABI-encoded MemberAdded event log
    fn create_member_added_log(plugin_address: Vec<u8>, dao_address: Vec<u8>, member_address: Vec<u8>) -> Log {
        // keccak256(MemberAdded(address,address))
        let event_signature = hex::decode("6a2af11b2d73f347f9d5840aea46899e17609730b5cd91bd9c312098038acba6").unwrap();

        let mut data = Vec::new();
        
        data.extend(vec![0u8; 12]);
        data.extend(&dao_address);
        
        data.extend(vec![0u8; 12]);
        data.extend(&member_address);

        Log {
            address: plugin_address,
            data,
            topics: vec![event_signature],
            index: 0,
            block_index: 0,
            ordinal: 0,
        }
    }

    /// Creates a properly ABI-encoded MembersAdded event log
    fn create_members_added_log(plugin_address: Vec<u8>, dao_address: Vec<u8>, member_addresses: Vec<Vec<u8>>) -> Log {
        // keccak256(MembersAdded(address,address[]))
        let event_signature = hex::decode("54f947227a98368ad184c035b39c0ee812a8a4777c500a475c985cef0e2de65c").unwrap();

        let mut data = Vec::new();
        
        data.extend(vec![0u8; 12]);
        data.extend(&dao_address);
        
        data.extend(vec![0u8; 31]);
        data.push(0x40);
        
        data.extend(vec![0u8; 31]);
        data.push(member_addresses.len() as u8);
        
        for member_address in &member_addresses {
            data.extend(vec![0u8; 12]);
            data.extend(member_address);
        }

        Log {
            address: plugin_address,
            data,
            topics: vec![event_signature],
            index: 0,
            block_index: 0,
            ordinal: 0,
        }
    }

    #[test]
    fn test_map_members_added_single_member() {
        // Given: A block with a single MemberAdded event
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        let member_address = vec![0x33; 20];
        
        let log = create_member_added_log(plugin_address.clone(), dao_address.clone(), member_address.clone());
        let block = create_mock_block_with_logs(vec![log]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: One member added
        assert_eq!(result.members.len(), 1);
        let member = &result.members[0];
        assert_eq!(member.change_type, "added");
        assert_eq!(member.main_voting_plugin_address, format_hex(&plugin_address));
        assert_eq!(member.member_address, format_hex(&member_address));
        assert_eq!(member.dao_address, format_hex(&dao_address));
    }

    #[test]
    fn test_map_members_added_batch_members() {
        // Given: A block with a single MembersAdded event containing multiple members
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        let member_addresses = vec![
            vec![0x33; 20],
            vec![0x44; 20],
            vec![0x55; 20],
        ];
        
        let log = create_members_added_log(plugin_address.clone(), dao_address.clone(), member_addresses.clone());
        let block = create_mock_block_with_logs(vec![log]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: Three members added
        assert_eq!(result.members.len(), 3);
        
        for (i, member) in result.members.iter().enumerate() {
            assert_eq!(member.change_type, "added");
            assert_eq!(member.main_voting_plugin_address, format_hex(&plugin_address));
            assert_eq!(member.member_address, format_hex(&member_addresses[i]));
            assert_eq!(member.dao_address, format_hex(&dao_address));
        }
    }

    #[test]
    fn test_map_members_added_mixed_events() {
        // Given: A block with both MemberAdded and MembersAdded events
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        
        let single_member = vec![0x33; 20];
        let single_log = create_member_added_log(plugin_address.clone(), dao_address.clone(), single_member.clone());
        
        let batch_members = vec![vec![0x44; 20], vec![0x55; 20]];
        let batch_log = create_members_added_log(plugin_address.clone(), dao_address.clone(), batch_members.clone());
        
        let block = create_mock_block_with_logs(vec![single_log, batch_log]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: Three members added total (1 from single + 2 from batch)
        assert_eq!(result.members.len(), 3);
        
        assert_eq!(result.members[0].member_address, format_hex(&single_member));
        assert_eq!(result.members[1].member_address, format_hex(&batch_members[0]));
        assert_eq!(result.members[2].member_address, format_hex(&batch_members[1]));
    }

    #[test]
    fn test_map_members_added_empty_block() {
        // Given: An empty block with no logs
        let block = create_mock_block_with_logs(vec![]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: No members added
        assert_eq!(result.members.len(), 0);
    }

    #[test]
    fn test_map_members_added_unrecognized_events() {
        // Given: A block with logs that don't match our event signatures
        let unrecognized_log = Log {
            address: vec![0x11; 20],
            data: vec![0x12, 0x34, 0x56, 0x78],
            topics: vec![vec![0x00; 32]],
            index: 0,
            block_index: 0,
            ordinal: 0,
        };
        let block = create_mock_block_with_logs(vec![unrecognized_log]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: No members added
        assert_eq!(result.members.len(), 0);
    }

    #[test]
    fn test_map_members_added_empty_batch() {
        // Given: A block with a MembersAdded event containing no members
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        let empty_members: Vec<Vec<u8>> = vec![];
        
        let log = create_members_added_log(plugin_address, dao_address, empty_members);
        let block = create_mock_block_with_logs(vec![log]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: No members added
        assert_eq!(result.members.len(), 0);
    }

    #[test]
    fn test_map_members_added_different_daos() {
        // Given: A block with member events from different DAOs
        let plugin_address = vec![0x11; 20];
        let dao1_address = vec![0x22; 20];
        let dao2_address = vec![0x77; 20];
        let member1_address = vec![0x33; 20];
        let member2_address = vec![0x44; 20];
        
        let log1 = create_member_added_log(plugin_address.clone(), dao1_address.clone(), member1_address.clone());
        let log2 = create_member_added_log(plugin_address.clone(), dao2_address.clone(), member2_address.clone());
        let block = create_mock_block_with_logs(vec![log1, log2]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: Two members added with correct DAO addresses
        assert_eq!(result.members.len(), 2);
        assert_eq!(result.members[0].dao_address, format_hex(&dao1_address));
        assert_eq!(result.members[1].dao_address, format_hex(&dao2_address));
    }

    #[test]
    fn test_map_members_added_large_batch() {
        // Given: A block with a MembersAdded event containing many members
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        let member_addresses: Vec<Vec<u8>> = (0..10)
            .map(|i| vec![i as u8; 20])
            .collect();
        
        let log = create_members_added_log(plugin_address.clone(), dao_address.clone(), member_addresses.clone());
        let block = create_mock_block_with_logs(vec![log]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: All 10 members added
        assert_eq!(result.members.len(), 10);
        
        for (i, member) in result.members.iter().enumerate() {
            assert_eq!(member.change_type, "added");
            assert_eq!(member.main_voting_plugin_address, format_hex(&plugin_address));
            assert_eq!(member.member_address, format_hex(&member_addresses[i]));
            assert_eq!(member.dao_address, format_hex(&dao_address));
        }
    }

    #[test]
    fn test_map_members_added_multiple_contracts() {
        // Given: A block with member events from different plugin contracts
        let plugin1_address = vec![0x11; 20];
        let plugin2_address = vec![0x88; 20];
        let dao_address = vec![0x22; 20];
        let member1_address = vec![0x33; 20];
        let member2_address = vec![0x44; 20];
        
        let log1 = create_member_added_log(plugin1_address.clone(), dao_address.clone(), member1_address.clone());
        let log2 = create_member_added_log(plugin2_address.clone(), dao_address.clone(), member2_address.clone());
        let block = create_mock_block_with_logs(vec![log1, log2]);

        // When: Processing the block
        let result = _map_members_added(block).expect("Failed to process block");

        // Expect: Two members added with correct plugin addresses
        assert_eq!(result.members.len(), 2);
        assert_eq!(result.members[0].main_voting_plugin_address, format_hex(&plugin1_address));
        assert_eq!(result.members[1].main_voting_plugin_address, format_hex(&plugin2_address));
    }

    /// Creates a properly ABI-encoded EditorAdded event log
    fn create_editor_added_log(plugin_address: Vec<u8>, dao_address: Vec<u8>, editor_address: Vec<u8>) -> Log {
        // keccak256(EditorAdded(address,address))
        let event_signature = hex::decode("9be2e3a904f17483f2325179628b7c166e45f79cc1dd501e98c8dea6f4437aca").unwrap();

        let mut data = Vec::new();
        
        data.extend(vec![0u8; 12]);
        data.extend(&dao_address);
        
        data.extend(vec![0u8; 12]);
        data.extend(&editor_address);

        Log {
            address: plugin_address,
            data,
            topics: vec![event_signature],
            index: 0,
            block_index: 0,
            ordinal: 0,
        }
    }

    /// Creates a properly ABI-encoded EditorsAdded event log
    fn create_editors_added_log(plugin_address: Vec<u8>, dao_address: Vec<u8>, editor_addresses: Vec<Vec<u8>>) -> Log {
        // keccak256(EditorsAdded(address,address[]))
        let event_signature = hex::decode("cbbd9049ad05566af71c386d03a5b5319c7ae2fdf47ef939238017b7e385440f").unwrap();

        let mut data = Vec::new();
        
        data.extend(vec![0u8; 12]);
        data.extend(&dao_address);
        
        data.extend(vec![0u8; 31]);
        data.push(0x40);
        
        data.extend(vec![0u8; 31]);
        data.push(editor_addresses.len() as u8);
        
        for editor_address in &editor_addresses {
            data.extend(vec![0u8; 12]);
            data.extend(editor_address);
        }

        Log {
            address: plugin_address,
            data,
            topics: vec![event_signature],
            index: 0,
            block_index: 0,
            ordinal: 0,
        }
    }

    #[test]
    fn test_map_editors_added_single_editor() {
        // Given: A block with a single EditorAdded event
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        let editor_address = vec![0x33; 20];
        
        let log = create_editor_added_log(plugin_address.clone(), dao_address.clone(), editor_address.clone());
        let block = create_mock_block_with_logs(vec![log]);

        // When: Processing the block
        let result = _map_editors_added(block).expect("Failed to process block");

        // Expect: One editor added
        assert_eq!(result.editors.len(), 1);
        let editor = &result.editors[0];
        assert_eq!(editor.change_type, "added");
        assert_eq!(editor.main_voting_plugin_address, format_hex(&plugin_address));
        assert_eq!(editor.editor_address, format_hex(&editor_address));
        assert_eq!(editor.dao_address, format_hex(&dao_address));
    }

    #[test]
    fn test_map_editors_added_multiple_editors() {
        // Given: A block with multiple EditorAdded events
        let plugin_address = vec![0x11; 20];
        let dao_address = vec![0x22; 20];
        let editor_addresses = vec![vec![0x33; 20], vec![0x44; 20]];

        let log = create_editors_added_log(plugin_address.clone(), dao_address.clone(), editor_addresses.clone());
        let block = create_mock_block_with_logs(vec![log]);

        // When: Processing the block
        let result = _map_editors_added(block).expect("Failed to process block");

        // Expect: Multiple editors added
        assert_eq!(result.editors.len(), 2);
        let editor1 = &result.editors[0];
        let editor2 = &result.editors[1];
        assert_eq!(editor1.change_type, "added");
        assert_eq!(editor1.main_voting_plugin_address, format_hex(&plugin_address));
        assert_eq!(editor1.editor_address, format_hex(&editor_addresses[0]));
        assert_eq!(editor1.dao_address, format_hex(&dao_address));
        assert_eq!(editor2.change_type, "added");
        assert_eq!(editor2.main_voting_plugin_address, format_hex(&plugin_address));
        assert_eq!(editor2.editor_address, format_hex(&editor_addresses[1]));
        assert_eq!(editor2.dao_address, format_hex(&dao_address));
    }

    #[test]
    fn test_map_editors_added_empty() {
        // Given: A block with no EditorAdded events
        let block = create_mock_block_with_logs(vec![]);

        // When: Processing the block
        let result = _map_editors_added(block).expect("Failed to process block");

        // Expect: No editors added
        assert_eq!(result.editors.len(), 0);
    }
}