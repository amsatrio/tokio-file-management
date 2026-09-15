use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FilterMatchMode {
    CONTAINS,
    SW, // START WITH
    EW, // END WITH
    BETWEEN,
    EQUALS,
    NOT,
    LT, // LESS THAN
    GT, // GREATER THAN
}
