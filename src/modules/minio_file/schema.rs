use chrono::NaiveDateTime;
use serde::Serialize;

use crate::modules::file::model::FileDb;
use crate::util::serializer::date_serializer;

#[derive(Serialize)]
pub struct FileUploadResponse {
    pub id: String,
}


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListResponse {
    pub id: String,
    pub file_name: String,
    pub stored_name: String,
    pub file_path: String,
    pub content_type: String,
    pub file_type: String,
    pub size: i64,
    #[serde(with = "date_serializer")]
    pub created_at: NaiveDateTime,
    #[serde(with = "date_serializer")]
    pub updated_at: NaiveDateTime,
}

impl From<&FileDb> for FileListResponse {
    fn from(file: &FileDb) -> Self {
        FileListResponse {
            id: file.id.clone(),
            file_name: file.file_name.clone(),
            stored_name: file.stored_name.clone(),
            file_path: file.file_path.clone(),
            content_type: file.content_type.clone(),
            file_type: file.file_type.clone(),
            size: file.size,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}