use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Serialize, Debug, Clone, Validate)]
pub struct Pagination {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub size: Option<i64>,
}
