use serde::Serialize;

#[derive(Serialize)]
pub struct FileUploadResponse {
    pub id: String,
}