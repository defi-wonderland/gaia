# Search Ranking Expectations

Manual test cases to verify that core Geo entities and types surface at the top of search results.

Root space ID: `d24e4d323f4eb6cc4eaa757cdd653857`

---

## Geo Root Space

| Search Term | Expected Top Result |
|---|---|
| `Geo` | Geo root space entity |
| `geo` | Geo root space (case insensitive) |

## Core Types

| Search Term | Expected Top Result |
|---|---|
| `Person` | Person type entity |
| `Space` | Space type entity |
| `Type` | Type schema entity |

## Renderable / Schema Types

| Search Term | Expected Top Result |
|---|---|
| `Image` | Image type entity |
| `Video` | Video type entity |
| `PDF` | PDF type entity |
| `URL` | URL type entity |
| `Place` | Place type entity |
| `Address` | Address type entity |
| `Text` | Text data type or Text Block |
| `Date` | Date data type |

## View Types

| Search Term | Expected Top Result |
|---|---|
| `Gallery` | Gallery view type |
| `Table` | Table view type |
| `List` | List view type |

## Fuzzy / Typo Tolerance

| Search Term | Expected Top Result |
|---|---|
| `Preson` | Person type (1-edit typo) |
| `Sapce` | Space type (transposition) |
| `Goe` | Geo root space (transposition) |

## Disambiguation

| Search Term | Expected Top Result |
|---|---|
| `Geo` | Root space should beat any user-created entity named "Geo" |
| `Wonderland` | Geo root space (if description contains "Wonderland") should rank appropriately vs other Wonderland entities |
| `Location` | Geo location type vs user entities with "location" in name |

## Important Entities

| Search Term | Entity ID | Space ID | Expected Top Result |
|---|---|---|---|
| `Yaniv` | `31cfe99fdf3549ef89094548f04858ff` | `a542cac04434987163d31071f3223af5` | Yaniv entity |
| `AI` | `8cb0a2b4adbf4627aa080cec5112099a` | `41e851610e13a19441c4d980f2f2ce6b` | AI space/entity |
| `Health` | `b97f07a619fd4ab0bb3d8296a8a26ab9` | `89bd89bf28ff8a0963faf92a8c905e20` | Health space/entity |
| `Crypto` | `0fcd62b5798f4078b84fa535ac95fcf3` | `c9f267dcb0d270718c2a3c45a64afd32` | Crypto space/entity |
| `World affairs` | `49fbca0730974581a9f0300d52fd22d6` | `89bd89bf28ff8a0963faf92a8c905e20` | World Affairs space/entity |
