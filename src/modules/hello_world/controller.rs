use http_body_util::BodyExt;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};

use crate::dto::response::app_response::{self, AppBody};
use crate::modules::hello_world::schema::{self, AppError};


pub fn plain_text() -> Result<Response<AppBody>, AppError> {
    let text = "hello world".to_string();
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain")
        .body(app_response::full_bytes(Bytes::from(text)))
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(response)
}

pub fn response_body() -> Result<Response<AppBody>, AppError> {
    Ok(app_response::ok(&"hello".to_string()))
}

pub fn path_param(name: String) -> Result<Response<AppBody>, AppError> {
    Ok(app_response::ok(&name))
}

pub fn query_param(params: schema::Params) -> Result<Response<AppBody>, AppError> {
    Ok(app_response::ok(&params))
}

pub async fn payload(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
    let body_bytes = req
        .into_body()
        .collect()
        .await
        .map_err(|_| AppError::BadRequest("failed to read body".to_string()))?
        .to_bytes();

    let parsed = serde_json::from_slice::<schema::Payloads>(&body_bytes)
        .map_err(|_| AppError::BadRequest("invalid payload".to_string()))?;

    Ok(app_response::ok(&parsed))
}

pub fn logger() -> Result<Response<AppBody>, AppError> {
    log::info!("logger");
    Ok(app_response::ok(&"logger".to_string()))
}

pub fn error(query_pairs: std::collections::HashMap<String, String>) -> Result<Response<AppBody>, AppError> {
    let error_code = query_pairs
        .get("code")
        .ok_or_else(|| AppError::BadRequest("missing 'code' query parameter".to_string()))?;

    match error_code.as_str() {
        "404" => Err(AppError::NotFound("not found".to_string())),
        "500" => Err(AppError::InternalServerError("internal server error".to_string())),
        _ => Err(AppError::BadRequest("bad request".to_string())),
    }
}