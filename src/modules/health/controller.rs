use http_body_util::Full;
use hyper::{Response, body::Bytes};

use crate::dto::response::app_response;
use crate::modules::hello_world::schema::AppError;

pub fn status() -> Result<Response<Full<Bytes>>, AppError> {
    Ok(app_response::ok(&"ok".to_string()))
}
