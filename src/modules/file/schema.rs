use crate::dto::enumerator::file_type::FileType;
use serde::Serialize;

#[derive(Serialize)]
pub struct FileUploadResponse {
    pub id: String,
}

pub fn guess_file_type(content_type: &str) -> FileType {
    if content_type.starts_with("image/") {
        FileType::IMAGE
    } else if content_type.starts_with("audio/") {
        FileType::AUDIO
    } else if content_type.starts_with("video/") {
        FileType::VIDEO
    } else if content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("pdf")
        || content_type.contains("msword")
        || content_type.contains("spreadsheet")
        || content_type.contains("presentation")
    {
        FileType::DOCUMENT
    } else {
        FileType::UNKNOWN
    }
}