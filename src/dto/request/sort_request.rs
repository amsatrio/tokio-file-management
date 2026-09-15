use serde::{Deserialize, Serialize};
use validator::Validate;
use crate::util::serializer::sorts_serializer;


#[derive(Deserialize, Serialize, Debug, Clone, Validate)]
pub struct Sort {
    pub id: String,
    pub desc: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, Validate)]
pub struct Sorts {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(with = "sorts_serializer")]
    pub sort: Option<Vec<Sort>>,
}
