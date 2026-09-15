use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FilterDataType {
    TEXT,
    NUMBER,
    DATE,
    BOOLEAN,
}
