// Data models for migration

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: String,
    pub created_at: String,
    pub created_at_block: String,
    pub updated_at: String,
    pub updated_at_block: String,
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub id: String,
    pub entity_id: String,
    pub type_id: String,
    pub from_entity_id: String,
    pub from_space_id: Option<String>,
    pub from_version_id: Option<String>,
    pub to_entity_id: String,
    pub to_space_id: Option<String>,
    pub to_version_id: Option<String>,
    pub position: Option<String>,
    pub space_id: String,
    pub verified: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Value {
    pub id: String,
    pub property_id: String,
    pub entity_id: String,
    pub space_id: String,
    pub string: Option<String>,
    pub boolean: Option<bool>,
    pub number: Option<String>,
    pub point: Option<String>,
    pub time: Option<String>,
    pub language: Option<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub id: String,
    pub data_type: String,
}

