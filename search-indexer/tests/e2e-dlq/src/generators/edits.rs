use anyhow::Result;
use prost::Message;
use uuid::Uuid;

use hermes_schema::pb::knowledge::HermesEdit;
use grc_20::{encode_edit, CreateEntity, DeleteEntity, Edit as Grc20Edit, Op as Grc20Op, PropertyValue, UnsetLanguage, UnsetValue, UpdateEntity, Value as Grc20Value};
use grc_20::model::RestoreEntity;

use sdk::core::ids::{AVATAR_PROPERTY_ID, DESCRIPTION_PROPERTY_ID, NAME_PROPERTY_ID};

/// Generate an UpdateEntity operation with name and description
pub fn create_entity_edit(
    edit_name: &str,
    space_id: Uuid,
    entity_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    avatar: Option<&str>,
) -> Result<Vec<u8>> {
    let mut set_properties = Vec::new();

    // Add name if provided
    if let Some(name_value) = name {
        set_properties.push(PropertyValue {
            property: *Uuid::parse_str(NAME_PROPERTY_ID)?.as_bytes(),
            value: Grc20Value::Text {
                value: name_value.into(),
                language: None,
            },
        });
    }

    // Add description if provided
    if let Some(desc_value) = description {
        set_properties.push(PropertyValue {
            property: *Uuid::parse_str(DESCRIPTION_PROPERTY_ID)?.as_bytes(),
            value: Grc20Value::Text {
                value: desc_value.into(),
                language: None,
            },
        });
    }

    // Add avatar if provided
    if let Some(avatar_value) = avatar {
        set_properties.push(PropertyValue {
            property: *Uuid::parse_str(AVATAR_PROPERTY_ID)?.as_bytes(),
            value: Grc20Value::Text {
                value: avatar_value.into(),
                language: None,
            },
        });
    }

    let update_entity = UpdateEntity {
        id: *entity_id.as_bytes(),
        set_properties,
        unset_values: vec![],
        context: None,
    };

    let grc20_edit = Grc20Edit {
        id: *Uuid::new_v4().as_bytes(),
        name: edit_name.into(),
        authors: vec![*Uuid::new_v4().as_bytes()],
        created_at: 0,
        ops: vec![Grc20Op::UpdateEntity(update_entity)],
    };

    // Encode the GRC-20 edit into bytes
    let payload = encode_edit(&grc20_edit)?;

    let edit = HermesEdit {
        id: grc20_edit.id.to_vec(),
        name: edit_name.to_string(),
        payload,
        authors: vec![Uuid::new_v4().as_bytes().to_vec()],
        language: None,
        space_id: space_id.as_bytes().to_vec(),
        is_canonical: true,
        meta: None,
    };

    let mut buf = Vec::new();
    edit.encode(&mut buf)?;
    Ok(buf)
}

/// Generate an edit that unsets entity properties
pub fn unset_entity_properties(
    edit_name: &str,
    space_id: Uuid,
    entity_id: Uuid,
    property_ids: Vec<&str>,
) -> Result<Vec<u8>> {
    let unset_values: Result<Vec<_>> = property_ids
        .into_iter()
        .map(|id| {
            Ok(UnsetValue {
                property: *Uuid::parse_str(id)?.as_bytes(),
                language: UnsetLanguage::All,
            })
        })
        .collect();

    let update_entity = UpdateEntity {
        id: *entity_id.as_bytes(),
        set_properties: vec![],
        unset_values: unset_values?,
        context: None,
    };

    let grc20_edit = Grc20Edit {
        id: *Uuid::new_v4().as_bytes(),
        name: edit_name.into(),
        authors: vec![*Uuid::new_v4().as_bytes()],
        created_at: 0,
        ops: vec![Grc20Op::UpdateEntity(update_entity)],
    };

    // Encode the GRC-20 edit into bytes
    let payload = encode_edit(&grc20_edit)?;

    let edit = HermesEdit {
        id: grc20_edit.id.to_vec(),
        name: edit_name.to_string(),
        payload,
        authors: vec![Uuid::new_v4().as_bytes().to_vec()],
        language: None,
        space_id: space_id.as_bytes().to_vec(),
        is_canonical: true,
        meta: None,
    };

    let mut buf = Vec::new();
    edit.encode(&mut buf)?;
    Ok(buf)
}

/// Generate a CreateEntity operation (using the actual GRC-20 CreateEntity op)
///
/// This is different from create_entity_edit which uses UpdateEntity.
/// CreateEntity initializes a new entity with optional property values.
pub fn create_entity_grc20_op(
    edit_name: &str,
    space_id: Uuid,
    entity_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    avatar: Option<&str>,
) -> Result<Vec<u8>> {
    let mut values = Vec::new();

    // Add name if provided
    if let Some(name_value) = name {
        values.push(PropertyValue {
            property: *Uuid::parse_str(NAME_PROPERTY_ID)?.as_bytes(),
            value: Grc20Value::Text {
                value: name_value.into(),
                language: None,
            },
        });
    }

    // Add description if provided
    if let Some(desc_value) = description {
        values.push(PropertyValue {
            property: *Uuid::parse_str(DESCRIPTION_PROPERTY_ID)?.as_bytes(),
            value: Grc20Value::Text {
                value: desc_value.into(),
                language: None,
            },
        });
    }

    // Add avatar if provided
    if let Some(avatar_value) = avatar {
        values.push(PropertyValue {
            property: *Uuid::parse_str(AVATAR_PROPERTY_ID)?.as_bytes(),
            value: Grc20Value::Text {
                value: avatar_value.into(),
                language: None,
            },
        });
    }

    let create_entity = CreateEntity {
        id: *entity_id.as_bytes(),
        values,
        context: None,
    };

    let grc20_edit = Grc20Edit {
        id: *Uuid::new_v4().as_bytes(),
        name: edit_name.into(),
        authors: vec![*Uuid::new_v4().as_bytes()],
        created_at: 0,
        ops: vec![Grc20Op::CreateEntity(create_entity)],
    };

    // Encode the GRC-20 edit into bytes
    let payload = encode_edit(&grc20_edit)?;

    let edit = HermesEdit {
        id: grc20_edit.id.to_vec(),
        name: edit_name.to_string(),
        payload,
        authors: vec![Uuid::new_v4().as_bytes().to_vec()],
        language: None,
        space_id: space_id.as_bytes().to_vec(),
        is_canonical: true,
        meta: None,
    };

    let mut buf = Vec::new();
    edit.encode(&mut buf)?;
    Ok(buf)
}

/// Generate a DeleteEntity operation
#[allow(dead_code)]
pub fn delete_entity(
    edit_name: &str,
    space_id: Uuid,
    entity_id: Uuid,
) -> Result<Vec<u8>> {
    let delete_entity = DeleteEntity {
        id: *entity_id.as_bytes(),
        context: None,
    };

    let grc20_edit = Grc20Edit {
        id: *Uuid::new_v4().as_bytes(),
        name: edit_name.into(),
        authors: vec![*Uuid::new_v4().as_bytes()],
        created_at: 0,
        ops: vec![Grc20Op::DeleteEntity(delete_entity)],
    };

    // Encode the GRC-20 edit into bytes
    let payload = encode_edit(&grc20_edit)?;

    let edit = HermesEdit {
        id: grc20_edit.id.to_vec(),
        name: edit_name.to_string(),
        payload,
        authors: vec![Uuid::new_v4().as_bytes().to_vec()],
        language: None,
        space_id: space_id.as_bytes().to_vec(),
        is_canonical: true,
        meta: None,
    };

    let mut buf = Vec::new();
    edit.encode(&mut buf)?;
    Ok(buf)
}

/// Generate a RestoreEntity operation (un-delete)
#[allow(dead_code)]
pub fn restore_entity(
    edit_name: &str,
    space_id: Uuid,
    entity_id: Uuid,
) -> Result<Vec<u8>> {
    let restore_entity = RestoreEntity {
        id: *entity_id.as_bytes(),
        context: None,
    };

    let grc20_edit = Grc20Edit {
        id: *Uuid::new_v4().as_bytes(),
        name: edit_name.into(),
        authors: vec![*Uuid::new_v4().as_bytes()],
        created_at: 0,
        ops: vec![Grc20Op::RestoreEntity(restore_entity)],
    };

    // Encode the GRC-20 edit into bytes
    let payload = encode_edit(&grc20_edit)?;

    let edit = HermesEdit {
        id: grc20_edit.id.to_vec(),
        name: edit_name.to_string(),
        payload,
        authors: vec![Uuid::new_v4().as_bytes().to_vec()],
        language: None,
        space_id: space_id.as_bytes().to_vec(),
        is_canonical: true,
        meta: None,
    };

    let mut buf = Vec::new();
    edit.encode(&mut buf)?;
    Ok(buf)
}
