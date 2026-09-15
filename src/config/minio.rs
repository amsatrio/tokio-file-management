use lazy_static::lazy_static;
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use minio::s3::response::BucketExistsResponse;
use minio::s3::types::{BucketName, S3Api};
use minio::s3::MinioClient;
use minio::s3::MinioClientBuilder;

use crate::dto::environment::Environment;
use crate::modules::hello_world::schema::AppError;

lazy_static! {
    pub static ref CLIENT: MinioClient = build_client().expect("failed to create MinIO client");
}

fn build_client() -> Result<MinioClient, minio::s3::error::Error> {
    let config: &Environment = &crate::config::environment::CONFIG;

    let base_url: BaseUrl = config.minio_endpoint.parse()?;
    let provider = StaticProvider::new(
        config.minio_access_key.as_str(),
        config.minio_secret_key.as_str(),
        None,
    );

    MinioClientBuilder::new(base_url)
        .provider(Some(provider))
        .build()
}

pub async fn ensure_bucket() -> Result<(), AppError> {
    let config: &Environment = &crate::config::environment::CONFIG;
    let bucket = BucketName::new(config.minio_bucket.as_str())
        .map_err(|e| AppError::InternalServerError(format!("invalid bucket name: {}", e)))?;

    let exists: BucketExistsResponse = CLIENT
        .bucket_exists(bucket.clone())
        .map_err(|e| AppError::InternalServerError(format!("bucket exists check failed: {}", e)))?
        .build()
        .send()
        .await
        .map_err(|e| AppError::InternalServerError(format!("bucket exists check failed: {}", e)))?;

    if !exists.exists() {
        CLIENT
            .create_bucket(bucket)
            .map_err(|e| AppError::InternalServerError(format!("create bucket failed: {}", e)))?
            .build()
            .send()
            .await
            .map_err(|e| AppError::InternalServerError(format!("create bucket failed: {}", e)))?;
        log::info!("MinIO bucket '{}' created", config.minio_bucket);
    }

    Ok(())
}