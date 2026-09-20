use hyper::Response;

use crate::dto::response::app_response::{self, AppBody};
use crate::modules::hello_world::schema::AppError;

pub fn status() -> Result<Response<AppBody>, AppError> {
    Ok(app_response::ok(&"ok".to_string()))
}