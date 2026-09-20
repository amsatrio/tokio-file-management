use hyper::body::Incoming;
use hyper::{Method, Request, Response};

use super::controller;
use crate::dto::response::app_response::{self, AppBody};
use crate::middleware::error_middleware::error_handler_with_fallback;

pub async fn handle_request(_req: Request<Incoming>) -> Result<Response<AppBody>, hyper::Error> {
    let method = _req.method().clone();
    let path = _req.uri().path().to_owned();

    let response = match (method, path.as_str()) {
        (Method::GET, "/v1/health/status") => {
            return error_handler_with_fallback(_req, |_req| async move {
                controller::status()
            })
            .await;
        }

        // 404 Fallback
        _ => app_response::not_found(),
    };

    Ok(response)
}