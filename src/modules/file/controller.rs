use std::collections::HashMap;

use http_body_util::{BodyDataStream, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{HeaderMap, Request, Response, StatusCode};
use multer::Multipart;
use tokio::fs;
use uuid::Uuid;

use crate::config::database;
use crate::modules::file::model::{self, FileDb, NewFile};
use crate::modules::file::schema::{self};
use crate::modules::hello_world::schema::AppError;

const UPLOAD_DIR: &str = "./uploads";

struct UploadedFile {
    file_name: String,
    content_type: String,
    bytes: Vec<u8>,
}

pub async fn upload(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let uploaded = extract_uploaded_file(req).await?;

    fs::create_dir_all(UPLOAD_DIR)
        .await
        .map_err(|e| AppError::InternalServerError(format!("failed to create upload dir: {}", e)))?;

    let file_id = Uuid::new_v4().to_string();
    let stored_name = stored_name_of(&file_id, &uploaded.file_name);
    let file_path = format!("{}/{}", UPLOAD_DIR, stored_name);

    fs::write(&file_path, &uploaded.bytes)
        .await
        .map_err(|e| AppError::InternalServerError(format!("failed to save file: {}", e)))?;

    let file_type = schema::guess_file_type(&uploaded.content_type).to_string();
    let size = uploaded.bytes.len() as i64;
    let file_id_clone = file_id.clone();

    let new_file = NewFile {
        id: file_id.clone(),
        file_name: uploaded.file_name,
        stored_name,
        file_path,
        content_type: uploaded.content_type,
        file_type,
        size,
    };

    tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;
        diesel::insert_into(model::files::table)
            .values(&new_file)
            .execute(&mut conn)
            .map_err(|e| AppError::InternalServerError(format!("failed to insert file record: {}", e)))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    log::info!("File uploaded: id={}", file_id_clone);

    Ok(app_ok(&schema::FileUploadResponse { id: file_id_clone }))
}

pub async fn update(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let file_id = query_id(&req)?;
    let uploaded = extract_uploaded_file(req).await?;

    let existing_path = tokio::task::spawn_blocking({
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
            Ok(existing.file_path)
        }
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    let stored_name = stored_name_of(&file_id, &uploaded.file_name);
    let new_path = format!("{}/{}", UPLOAD_DIR, stored_name);

    fs::write(&new_path, &uploaded.bytes)
        .await
        .map_err(|e| AppError::InternalServerError(format!("failed to save updated file: {}", e)))?;

    let file_type = schema::guess_file_type(&uploaded.content_type).to_string();
    let size = uploaded.bytes.len() as i64;

    let new_path_for_db = new_path.clone();
    let file_id_for_db = file_id.clone();
    tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;
        diesel::update(model::files::table.find(&file_id_for_db))
            .set((
                model::files::file_name.eq(&uploaded.file_name),
                model::files::stored_name.eq(&stored_name),
                model::files::file_path.eq(&new_path_for_db),
                model::files::content_type.eq(&uploaded.content_type),
                model::files::file_type.eq(&file_type),
                model::files::size.eq(size),
            ))
            .execute(&mut conn)
            .map_err(|e| AppError::InternalServerError(format!("failed to update file record: {}", e)))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    if existing_path != new_path {
        remove_file(&existing_path).await;
    }

    log::info!("File updated: id={}", file_id);

    Ok(app_ok(&schema::FileUploadResponse { id: file_id }))
}

pub async fn delete(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, AppError> {
    let file_id = query_id(&req)?;

    let file_path = tokio::task::spawn_blocking({
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
            Ok(existing.file_path)
        }
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    remove_file(&file_path).await;

    log::info!("File deleted: id={}", file_id);

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

    let file_bytes = fs::read(&file_db.file_path)
        .await
        .map_err(|e| AppError::NotFound(format!("file not found on disk: {}", e)))?;

    let content_disposition = format!(
        "attachment; filename=\"{}\"",
        escape_quotes(&file_db.file_name)
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", file_db.content_type)
        .header("content-length", file_bytes.len().to_string())
        .header("content-disposition", content_disposition)
        .body(Full::new(Bytes::from(file_bytes)))
        .map_err(|e| AppError::InternalServerError(e.to_string()))?)
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

async fn remove_file(path: &str) {
    if let Err(e) = fs::remove_file(path).await {
        log::warn!("failed to remove file {}: {}", path, e);
    }
}

fn escape_quotes(value: &str) -> String {
    value.replace('"', "\\\"")
}

fn app_ok<T: serde::Serialize>(data: &T) -> Response<Full<Bytes>> {
    crate::dto::response::app_response::ok(data)
}