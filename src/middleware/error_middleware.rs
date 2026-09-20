use hyper::body::Incoming;
use hyper::{Request, Response};

use crate::dto::response::app_response::AppBody;
use crate::modules::hello_world::schema::AppError;

pub async fn error_handler<F, Fut>(
    req: Request<Incoming>,
    handler: F,
) -> Result<Response<AppBody>, AppError>
where
    F: FnOnce(Request<Incoming>) -> Fut,
    Fut: std::future::Future<Output = Result<Response<AppBody>, AppError>>,
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
) -> Result<Response<AppBody>, hyper::Error>
where
    F: FnOnce(Request<Incoming>) -> Fut,
    Fut: std::future::Future<Output = Result<Response<AppBody>, AppError>>,
{
    match error_handler(req, handler).await {
        Ok(response) => Ok(response),
        Err(app_error) => Ok(app_error.to_response()),
    }
}