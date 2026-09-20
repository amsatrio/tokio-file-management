use hyper::body::Incoming;
use hyper::{Method, Request, Response};

use super::controller;
use crate::dto::response::app_response::{self, AppBody};
use crate::middleware::error_middleware::error_handler_with_fallback;

pub async fn handle_request(req: Request<Incoming>) -> Result<Response<AppBody>, hyper::Error> {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();

    match (method, path.as_str()) {
        (Method::POST, "/v1/file/upload") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::upload(_req).await
            })
            .await;
        }

        (Method::PUT, "/v1/file/update") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::update(_req).await
            })
            .await;
        }

        (Method::DELETE, "/v1/file/delete") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::delete(_req).await
            })
            .await;
        }

        (Method::GET, "/v1/file/download") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::download(_req).await
            })
            .await;
        }

        (Method::GET, "/v1/file/stream") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::stream(_req).await
            })
            .await;
        }

        (Method::GET, "/v1/file/pagination") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::pagination(_req).await
            })
            .await;
        }

        _ => return Ok(app_response::not_found()),
    };
}