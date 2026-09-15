use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

use crate::modules::hello_world::schema::AppError;

pub async fn error_handler<F, Fut>(
    req: Request<Incoming>,
    handler: F,
) -> Result<Response<Full<Bytes>>, AppError>
where
    F: FnOnce(Request<Incoming>) -> Fut,
    Fut: std::future::Future<Output = Result<Response<Full<Bytes>>, AppError>>,
{
    match handler(req).await {
        Ok(response) => Ok(response),
        Err(app_error) => {
            log::error!(
                "[APP ERROR] {} - {}",
                app_error.status_code().as_u16(),
                app_error
            );
            Err(app_error)
        }
    }
}

pub async fn error_handler_with_fallback<F, Fut>(
    req: Request<Incoming>,
    handler: F,
) -> Result<Response<Full<Bytes>>, hyper::Error>
where
    F: FnOnce(Request<Incoming>) -> Fut,
    Fut: std::future::Future<Output = Result<Response<Full<Bytes>>, AppError>>,
{
    match error_handler(req, handler).await {
        Ok(response) => Ok(response),
        Err(app_error) => Ok(app_error.to_response()),
    }
}
