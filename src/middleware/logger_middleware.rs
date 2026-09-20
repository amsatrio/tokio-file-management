use std::time::Instant;

use hyper::body::Incoming;
use hyper::{Request, Response};
use log::Level;

use crate::dto::response::app_response::AppBody;

pub async fn logging_handler<F, Fut>(
    req: Request<Incoming>,
    handler: F,
) -> Result<Response<AppBody>, hyper::Error>
where
    F: FnOnce(Request<Incoming>) -> Fut,
    Fut: std::future::Future<Output = Result<Response<AppBody>, hyper::Error>>,
{
    let method = req.method().clone();
    let uri = req.uri().clone();
    let version = req.version();
    let headers = req.headers().clone();

    log_request(&method, &uri, version, &headers);

    let start = Instant::now();
    let result = handler(req).await;
    let elapsed = start.elapsed();

    match &result {
        Ok(resp) => {
            log_response(&method, &uri, resp, elapsed);
        }
        Err(err) => {
            log::error!(
                "[REQUEST ERROR] {} {} -> error={:?} elapsed={}ms",
                method,
                uri,
                err,
                elapsed.as_millis()
            );
        }
    }

    result
}

fn log_request(
    method: &hyper::Method,
    uri: &hyper::Uri,
    version: hyper::Version,
    headers: &hyper::HeaderMap,
) {
    log::log!(
        Level::Info,
        "[INCOMING] {} {} {:?}",
        method,
        uri,
        version
    );

    if log::log_enabled!(Level::Debug) {
        for (name, value) in headers.iter() {
            if let Ok(v) = value.to_str() {
                log::debug!("[INCOMING HEADER] {}: {}", name, v);
            }
        }
    }
}

fn log_response(
    method: &hyper::Method,
    uri: &hyper::Uri,
    resp: &Response<AppBody>,
    elapsed: std::time::Duration,
) {
    let status = resp.status();
    log::log!(
        Level::Info,
        "[OUTGOING] {} {} -> {} {} elapsed={}ms",
        method,
        uri,
        status.as_u16(),
        status.canonical_reason().unwrap_or(""),
        elapsed.as_millis()
    );

    if log::log_enabled!(Level::Debug) {
        for (name, value) in resp.headers().iter() {
            if let Ok(v) = value.to_str() {
                log::debug!("[OUTGOING HEADER] {}: {}", name, v);
            }
        }
    }
}