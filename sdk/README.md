# SDK

A Rust SDK for interacting with The Graph knowledge graph.

## Overview

Entities in The Graph have attributes that provide semantic meaning. Each type of attribute has a unique identifier. This SDK provides constants for well-known attribute IDs, such as name and description attributes.

## Usage

### Entity Attribute IDs

```rust
use sdk::core::ids::{NAME_ATTRIBUTE, DESCRIPTION_ATTRIBUTE};

// Use these constants when creating or querying entity attributes
let name_attribute_id = NAME_ATTRIBUTE;
let description_attribute_id = DESCRIPTION_ATTRIBUTE;
```

