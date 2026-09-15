use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub total_elements: i64,
    pub total_pages: i64,
    pub content: Vec<T>,
}
