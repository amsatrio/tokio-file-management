use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio_file_management::config::database;
use tokio_file_management::config::environment::CONFIG;
use tokio_file_management::config::logger;
use tokio_file_management::dto::response::app_response;
use tokio_file_management::middleware::logger_middleware::logging_handler;
use tokio_file_management::modules::{
    file::router as file_router, health::router as health_router,
    hello_world::router as hello_world_router, minio_file::router as minio_file_router,
};

async fn route(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path().to_owned();

    if path.starts_with("/v1/file") {
        return file_router::handle_request(req).await;
    }

    if path.starts_with("/v1/minio-file") {
        return minio_file_router::handle_request(req).await;
    }

    if path.starts_with("/v1/health") {
        return health_router::handle_request(req).await;
    }

    if path.starts_with("/v1/hello-world") {
        return hello_world_router::handle_request(req).await;
    }

    Ok(app_response::not_found())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // logger init
    let _logger = logger::main::initialize();
    
    // config init
    let config = &CONFIG;

    // database init
    database::run_migrations();
    
    // let addr = SocketAddr::from(([127, 0, 0, 1], 8009));
    let addr = format!("{}:{}", config.server_host, config.server_port);
    let listener = TcpListener::bind(addr.clone()).await?;
    log::info!("Server running on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(|req| logging_handler(req, route)))
                .await
            {
                log::error!("Error serving connection: {:?}", err);
            }
        });
    }
}