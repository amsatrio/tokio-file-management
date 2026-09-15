use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    dto::enumerator::{filter_data_type::FilterDataType, filter_match_mode::FilterMatchMode},
    util::serializer::filters_serializer,
};

#[derive(Deserialize, Serialize, Debug, Clone, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub id: String,
    pub value: String,
    pub match_mode: FilterMatchMode,
    pub data_type: FilterDataType,
}

#[derive(Deserialize, Serialize, Debug, Clone, Validate)]
pub struct Filters {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(with = "filters_serializer")]
    pub filter: Option<Vec<Filter>>,
}
