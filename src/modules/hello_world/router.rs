use std::collections::HashMap;

use hyper::body::Incoming;
use hyper::{Method, Request, Response};
use url::Url;

use super::controller;
use crate::dto::response::app_response::{self, AppBody};
use crate::middleware::error_middleware::error_handler_with_fallback;
use crate::modules::hello_world::schema::{self, AppError};

pub async fn handle_request(req: Request<Incoming>) -> Result<Response<AppBody>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();

    let dummy_url = format!("http://localhost{}", uri);
    let parsed_url = Url::parse(&dummy_url).unwrap();

    let response = match (method, path) {
        (Method::GET, "/v1/hello-world") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::plain_text()
            })
            .await;
        }

        (Method::GET, "/v1/hello-world/response-body") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::response_body()
            })
            .await;
        }

        (Method::GET, p) if p.starts_with("/v1/hello-world/path/") => {
            let name = p
                .strip_prefix("/v1/hello-world/path/")
                .unwrap_or("World")
                .to_string();
            return error_handler_with_fallback(req, move |_req| async move {
                controller::path_param(name)
            })
            .await;
        }

        (Method::GET, "/v1/hello-world/query") => {
            let query = uri.query().unwrap_or("");
            return error_handler_with_fallback(req, move |_req| async move {
                match serde_urlencoded::from_str::<schema::Params>(query) {
                    Ok(params) => controller::query_param(params),
                    Err(_) => Err(AppError::BadRequest("invalid query param".to_string())),
                }
            })
            .await;
        }

        (Method::POST, "/v1/hello-world/payload") => {
            return error_handler_with_fallback(req, controller::payload).await;
        }

        (Method::GET, "/v1/hello-world/logger") => {
            return error_handler_with_fallback(req, |_req| async move {
                controller::logger()
            })
            .await;
        }

        (Method::GET, "/v1/hello-world/error") => {
            let query_pairs: HashMap<_, _> = parsed_url.query_pairs().into_owned().collect();
            return error_handler_with_fallback(req, move |_req| async move {
                controller::error(query_pairs)
            })
            .await;
        }

        // 404 Fallback
        _ => app_response::not_found(),
    };

    Ok(response)
}
