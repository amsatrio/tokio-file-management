use std::collections::HashMap;

use http_body_util::{BodyDataStream, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{HeaderMap, Request, Response, StatusCode};
use minio::s3::builders::ObjectContent;
use minio::s3::types::{BucketName, ObjectKey, S3Api};
use multer::Multipart;
use uuid::Uuid;

use crate::config::database;
use crate::config::environment::CONFIG;
use crate::config::minio::CLIENT;
use crate::dto::environment::Environment;
use crate::modules::file::model::{self, FileDb, NewFile};
use crate::modules::file::schema as file_schema;
use crate::modules::hello_world::schema::AppError;
use crate::modules::minio_file::schema;

struct UploadedFile {
    file_name: String,
    content_type: String,
    bytes: Vec<u8>,
}

pub async fn upload(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let uploaded = extract_uploaded_file(req).await?;

    create_bucket_if_missing().await?;

    let file_id = Uuid::new_v4().to_string();
    let stored_name = stored_name_of(&file_id, &uploaded.file_name);

    put_object(&stored_name, uploaded.bytes.clone()).await?;

    let file_type = file_schema::guess_file_type(&uploaded.content_type).to_string();
    let size = uploaded.bytes.len() as i64;

    let new_file = NewFile {
        id: file_id.clone(),
        file_name: uploaded.file_name,
        stored_name: stored_name.clone(),
        file_path: stored_name.clone(),
        content_type: uploaded.content_type,
        file_type,
        size,
    };

    let cleanup_key = stored_name.clone();
    let insert_result: Result<(), AppError> = tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;
        diesel::insert_into(model::files::table)
            .values(&new_file)
            .execute(&mut conn)
            .map_err(|e| AppError::InternalServerError(format!("failed to insert file record: {}", e)))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))?;

    if let Err(err) = insert_result {
        if let Err(rollback_err) = remove_object(&cleanup_key).await {
            log::warn!("failed to roll back object '{}': {}", cleanup_key, rollback_err);
        }
        return Err(err);
    }

    log::info!("File uploaded to MinIO: id={}", file_id);

    Ok(app_ok(&schema::FileUploadResponse { id: file_id }))
}

pub async fn update(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let file_id = query_id(&req)?;
    let uploaded = extract_uploaded_file(req).await?;

    let existing: FileDb = tokio::task::spawn_blocking({
        let file_id = file_id.clone();
        move || {
            use diesel::prelude::*;
            let mut conn = database::get_conn()?;
            model::files::table
                .find(&file_id)
                .select(FileDb::as_select())
                .first(&mut conn)
                .map_err(|e| match e {
                    diesel::result::Error::NotFound => {
                        AppError::NotFound(format!("file with id '{}' not found", file_id.clone()))
                    }
                    e => AppError::InternalServerError(format!("db query error: {}", e)),
                })
        }
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    let stored_name = stored_name_of(&file_id, &uploaded.file_name);

    put_object(&stored_name, uploaded.bytes.clone()).await?;

    let stored_name_for_db = stored_name.clone();
    let new_path_for_db = stored_name.clone();
    let file_id_for_db = file_id.clone();
    tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;
        diesel::update(model::files::table.find(&file_id_for_db))
            .set((
                model::files::file_name.eq(&uploaded.file_name),
                model::files::stored_name.eq(&stored_name_for_db),
                model::files::file_path.eq(&new_path_for_db),
                model::files::content_type.eq(&uploaded.content_type),
                model::files::file_type.eq(&file_schema::guess_file_type(&uploaded.content_type).to_string()),
                model::files::size.eq(uploaded.bytes.len() as i64),
            ))
            .execute(&mut conn)
            .map_err(|e| AppError::InternalServerError(format!("failed to update file record: {}", e)))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    if existing.stored_name != stored_name {
        if let Err(err) = remove_object(&existing.stored_name).await {
            log::warn!("failed to remove old object '{}': {}", existing.stored_name, err);
        }
    }

    log::info!("File updated in MinIO: id={}", file_id);

    Ok(app_ok(&schema::FileUploadResponse { id: file_id }))
}

pub async fn delete(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let file_id = query_id(&req)?;

    let stored_name = tokio::task::spawn_blocking({
        let file_id = file_id.clone();
        move || -> Result<String, AppError> {
            use diesel::prelude::*;
            let mut conn = database::get_conn()?;
            let existing: FileDb = model::files::table
                .find(&file_id)
                .select(FileDb::as_select())
                .first(&mut conn)
                .map_err(|e| match e {
                    diesel::result::Error::NotFound => {
                        AppError::NotFound(format!("file with id '{}' not found", file_id.clone()))
                    }
                    e => AppError::InternalServerError(format!("db query error: {}", e)),
                })?;
            diesel::delete(model::files::table.find(&file_id))
                .execute(&mut conn)
                .map_err(|e| AppError::InternalServerError(format!("failed to delete file record: {}", e)))?;
            Ok(existing.stored_name)
        }
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    if let Err(err) = remove_object(&stored_name).await {
        log::warn!("failed to remove object '{}': {}", stored_name, err);
    }

    log::info!("File deleted from MinIO: id={}", file_id);

    Ok(app_ok(&schema::FileUploadResponse { id: file_id }))
}

pub async fn download(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let file_id = query_id(&req)?;

    let file_db: FileDb = tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;
        model::files::table
            .find(&file_id)
            .select(FileDb::as_select())
            .first(&mut conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => {
                    AppError::NotFound(format!("file with id '{}' not found", file_id.clone()))
                }
                e => AppError::InternalServerError(format!("db query error: {}", e)),
            })
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    let object_bytes = get_object(&file_db.stored_name).await?;

    let content_disposition = format!(
        "attachment; filename=\"{}\"",
        escape_quotes(&file_db.file_name)
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", file_db.content_type)
        .header("content-length", object_bytes.len().to_string())
        .header("content-disposition", content_disposition)
        .body(Full::new(object_bytes))
        .map_err(|e| AppError::InternalServerError(e.to_string()))?)
}

async fn create_bucket_if_missing() -> Result<(), AppError> {
    let config: &Environment = &CONFIG;
    let bucket = BucketName::new(config.minio_bucket.as_str())
        .map_err(|e| AppError::InternalServerError(format!("invalid bucket name: {}", e)))?;

    let exists = CLIENT
        .bucket_exists(bucket.clone())
        .map_err(|e| AppError::InternalServerError(format!("bucket exists check failed: {}", e)))?
        .build()
        .send()
        .await
        .map_err(|e| AppError::InternalServerError(format!("bucket exists check failed: {}", e)))?
        .exists();

    if !exists {
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

async fn put_object(object_key: &str, bytes: Vec<u8>) -> Result<(), AppError> {
    let config: &Environment = &CONFIG;
    let bucket = BucketName::new(config.minio_bucket.as_str())
        .map_err(|e| AppError::InternalServerError(format!("invalid bucket name: {}", e)))?;
    let object = ObjectKey::new(object_key)
        .map_err(|e| AppError::InternalServerError(format!("invalid object key: {}", e)))?;

    CLIENT
        .put_object_content(bucket, object, ObjectContent::from(bytes))
        .map_err(|e| AppError::InternalServerError(format!("put object failed: {}", e)))?
        .build()
        .send()
        .await
        .map_err(|e| AppError::InternalServerError(format!("put object failed: {}", e)))?;

    Ok(())
}

async fn get_object(object_key: &str) -> Result<Bytes, AppError> {
    let config: &Environment = &CONFIG;
    let bucket = BucketName::new(config.minio_bucket.as_str())
        .map_err(|e| AppError::InternalServerError(format!("invalid bucket name: {}", e)))?;
    let object = ObjectKey::new(object_key)
        .map_err(|e| AppError::InternalServerError(format!("invalid object key: {}", e)))?;

    CLIENT
        .get_object(bucket, object)
        .map_err(|e| AppError::InternalServerError(format!("get object failed: {}", e)))?
        .build()
        .send()
        .await
        .map_err(|e| AppError::InternalServerError(format!("get object failed: {}", e)))?
        .into_bytes()
        .await
        .map_err(|e| AppError::InternalServerError(format!("get object failed: {}", e)))
}

async fn remove_object(object_key: &str) -> Result<(), AppError> {
    let config: &Environment = &CONFIG;
    let bucket = BucketName::new(config.minio_bucket.as_str())
        .map_err(|e| AppError::InternalServerError(format!("invalid bucket name: {}", e)))?;

    CLIENT
        .delete_object(bucket, object_key)
        .map_err(|e| AppError::InternalServerError(format!("delete object failed: {}", e)))?
        .build()
        .send()
        .await
        .map_err(|e| AppError::InternalServerError(format!("delete object failed: {}", e)))?;

    Ok(())
}

async fn extract_uploaded_file(req: Request<Incoming>) -> Result<UploadedFile, AppError> {
    let boundary = extract_boundary(req.headers()).ok_or_else(|| {
        AppError::BadRequest("missing multipart boundary in Content-Type header".to_string())
    })?;

    let multipart = Multipart::new(BodyDataStream::new(req.into_body()), boundary);

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::new();
    let mut content_type = String::new();

    let mut stream = multipart;
    while let Some(field) = stream
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("failed to read multipart field: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name != "file" {
            continue;
        }

        file_name = field.file_name().unwrap_or("file").to_string();
        content_type = field
            .content_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("failed to read file bytes: {}", e)))?;
        file_bytes = Some(bytes.to_vec());
        break;
    }

    let bytes = file_bytes.ok_or_else(|| {
        AppError::BadRequest("no file field found in multipart form data".to_string())
    })?;

    if bytes.is_empty() {
        return Err(AppError::BadRequest("uploaded file is empty".to_string()));
    }

    Ok(UploadedFile {
        file_name,
        content_type,
        bytes,
    })
}

fn query_id(req: &Request<Incoming>) -> Result<String, AppError> {
    let query = req.uri().query().unwrap_or("");
    let params: HashMap<String, String> = serde_urlencoded::from_str(query)
        .map_err(|_| AppError::BadRequest("invalid query param".to_string()))?;

    params
        .get("id")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("missing 'id' query parameter".to_string()))
}

fn extract_boundary(headers: &HeaderMap) -> Option<String> {
    headers.get("content-type")?.to_str().ok().and_then(|value| {
        value
            .split(';')
            .map(str::trim)
            .find_map(|part| part.strip_prefix("boundary="))
            .map(|b| b.trim_matches('"').to_string())
    })
}

fn stored_name_of(file_id: &str, file_name: &str) -> String {
    format!("{}.{}", file_id, extension_of(file_name))
}

fn extension_of(file_name: &str) -> String {
    file_name
        .rsplit_once('.')
        .map(|(_, ext)| {
            let ext = ext.trim().to_lowercase();
            if ext.is_empty() {
                "bin".to_string()
            } else {
                ext
            }
        })
        .unwrap_or_else(|| "bin".to_string())
}

fn escape_quotes(value: &str) -> String {
    value.replace('"', "\\\"")
}

fn app_ok<T: serde::Serialize>(data: &T) -> Response<Full<Bytes>> {
    crate::dto::response::app_response::ok(data)
}