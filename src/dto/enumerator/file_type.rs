use std::fmt;

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
#[serde(field_identifier, rename_all = "lowercase")]
pub enum FileType {
    DOCUMENT,
    IMAGE,
    AUDIO,
    VIDEO,
    UNKNOWN

}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::DOCUMENT => write!(f, "document"),
            FileType::IMAGE => write!(f, "image"),
            FileType::AUDIO => write!(f, "audio"),
            FileType::VIDEO => write!(f, "video"),
            FileType::UNKNOWN => write!(f, "unknown"),
        }
    }
}