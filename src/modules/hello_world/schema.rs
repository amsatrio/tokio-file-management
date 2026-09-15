use std::fmt;

use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes};
use serde::{Deserialize, Serialize};

use crate::dto::response::app_response;

#[derive(Deserialize, Serialize)]
pub struct Params {
    name: String,
    age: Option<u32>,
}


#[derive(Deserialize, Serialize)]
pub struct Payloads {
    name: String,
    age: Option<u32>,
}

#[derive(Deserialize, Serialize)]
pub struct ErrorType {
    pub(crate) code: i32, // 200, 400, 404, 500
}

#[derive(Deserialize)]
pub struct ParamsFile {
    pub(crate) file_path: String,
    pub(crate) file_name: String
}

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    InternalServerError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            AppError::InternalServerError(msg) => write!(f, "Internal Server Error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            AppError::NotFound(msg) => msg,
            AppError::BadRequest(msg) => msg,
            AppError::InternalServerError(msg) => msg,
        }
    }

    pub fn to_response(&self) -> Response<Full<Bytes>> {
        app_response::error(self.status_code(), &self.message().to_string())
    }
}