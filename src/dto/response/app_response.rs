use chrono::NaiveDateTime;
use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes};
use serde::{Deserialize, Serialize};

use crate::util::serializer::date_serializer;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppResponse<T> {
    pub status: u16,
    pub message: String,
    #[serde(with = "date_serializer")]
    pub timestamp: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<T>,
}
pub fn ok<T: Serialize>(data: &T) -> Response<Full<Bytes>> {
    let status_code = StatusCode::OK;

    let res = AppResponse {
        status: status_code.as_u16(),
        message: "success".to_owned(),
        timestamp: chrono::Utc::now().naive_utc(),
        data: Some(data),
        error: None,
    };

    let json_bytes = serde_json::to_vec(&res).unwrap();
    Response::builder()
        .status(status_code)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json_bytes)))
        .unwrap()
}

pub fn success<T: Serialize>(status_code: StatusCode, data: &T) -> Response<Full<Bytes>> {
    let res = AppResponse {
        status: status_code.as_u16(),
        message: "success".to_owned(),
        timestamp: chrono::Utc::now().naive_utc(),
        data: Some(data),
        error: None,
    };

    let json_bytes = serde_json::to_vec(&res).unwrap();
    Response::builder()
        .status(status_code)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json_bytes)))
        .unwrap()
}

pub fn error<T: Serialize>(status_code: StatusCode, err: &T) -> Response<Full<Bytes>> {
    let res = AppResponse {
        status: status_code.as_u16(),
        message: "error".to_owned(),
        timestamp: chrono::Utc::now().naive_utc(),
        data: None,
        error: Some(err),
    };

    let json_bytes = serde_json::to_vec(&res).unwrap();
    Response::builder()
        .status(status_code)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json_bytes)))
        .unwrap()
}

pub fn not_found() -> Response<Full<Bytes>> {
    error(StatusCode::NOT_FOUND, &"not found".to_string())
}
