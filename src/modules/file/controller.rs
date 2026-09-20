use std::collections::HashMap;

use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;
use futures_util::TryStreamExt;
use http_body_util::{BodyDataStream, BodyExt, StreamBody};
use hyper::body::{Bytes, Frame, Incoming};
use hyper::{HeaderMap, Request, Response, StatusCode};
use multer::Multipart;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

use crate::config::database;
use crate::dto::enumerator::filter_match_mode::FilterMatchMode;
use crate::dto::request::filter_request::Filters;
use crate::dto::request::pagination_request::Pagination;
use crate::dto::request::search_request::Search;
use crate::dto::request::sort_request::Sorts;
use crate::dto::response::app_response::{self, AppBody, AppErrorBody};
use crate::dto::response::pagination_response::PaginatedResponse;
use crate::modules::file::model::{self, FileDb, NewFile};
use crate::modules::file::schema::{self};
use crate::modules::hello_world::schema::AppError;

type FilesBaseQuery = model::files::BoxedQuery<'static, diesel::mysql::Mysql>;
type FilesPredicate = Box<dyn diesel::BoxableExpression<model::files::table, diesel::mysql::Mysql, SqlType = diesel::sql_types::Bool>>;

const UPLOAD_DIR: &str = "./uploads";

struct UploadedFile {
    file_name: String,
    content_type: String,
    bytes: Vec<u8>,
}

pub async fn upload(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
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

pub async fn update(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
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

pub async fn delete(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
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

pub async fn download(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
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
        .body(app_response::full_bytes(file_bytes))
        .map_err(|e| AppError::InternalServerError(e.to_string()))?)
}

pub async fn stream(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
    let file_id = query_id(&req)?;
    let file_db = find_file(&file_id).await?;

    let total = file_db.size.max(0) as u64;

    let range_header = req
        .headers()
        .get(hyper::header::RANGE)
        .and_then(|v| v.to_str().ok());

    let (status, start, end) = match parse_range(range_header, total) {
        Range::Full => (StatusCode::OK, 0, total.saturating_sub(1)),
        Range::Partial { start, end } => (StatusCode::PARTIAL_CONTENT, start, end),
        Range::Unsatisfiable => return range_not_satisfiable(total),
    };

    let length = if total == 0 { 0 } else { end - start + 1 };

    let mut builder = Response::builder()
        .status(status)
        .header("content-type", file_db.content_type.as_str())
        .header("accept-ranges", "bytes")
        .header("content-length", length.to_string())
        .header(
            "content-disposition",
            format!("inline; filename=\"{}\"", escape_quotes(&file_db.file_name)),
        );

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header("content-range", format!("bytes {}-{}/{}", start, end, total));
    }

    let body = stream_disk_body(&file_db.file_path, start, length).await?;

    Ok(builder.body(body).map_err(|e| AppError::InternalServerError(e.to_string()))?)
}

async fn stream_disk_body(path: &str, start: u64, length: u64) -> Result<AppBody, AppError> {
    let mut file = fs::File::open(path)
        .await
        .map_err(|e| AppError::NotFound(format!("file not found on disk: {}", e)))?;

    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| AppError::InternalServerError(format!("failed to seek file: {}", e)))?;

    let reader = file.take(length);
    let frames = tokio_util::io::ReaderStream::new(reader).map_ok(Frame::data);
    let stream_body = StreamBody::new(frames);
    let body: AppBody = BodyExt::map_err(stream_body, |e| -> AppErrorBody { e.into() }).boxed_unsync();

    Ok(body)
}

pub async fn pagination(req: Request<Incoming>) -> Result<Response<AppBody>, AppError> {
    let query = req.uri().query().unwrap_or("");

    let pagination: Pagination = serde_urlencoded::from_str(query)
        .map_err(|_| AppError::BadRequest("invalid query param".to_string()))?;
    let search: Search = serde_urlencoded::from_str(query)
        .map_err(|_| AppError::BadRequest("invalid query param".to_string()))?;
    let sorts: Sorts = serde_urlencoded::from_str(query)
        .map_err(|_| AppError::BadRequest("invalid query param".to_string()))?;
    let filters: Filters = serde_urlencoded::from_str(query)
        .map_err(|_| AppError::BadRequest("invalid query param".to_string()))?;

    let page = pagination.page.unwrap_or(0).max(0);
    let size = pagination.size.unwrap_or(10).clamp(1, 100);
    let offset = page * size;

    let search_term = search.search.filter(|s| !s.trim().is_empty());
    let sort_list = sorts.sort.unwrap_or_default();
    let filter_list = filters.filter.unwrap_or_default();

    let (total, rows) = tokio::task::spawn_blocking(move || -> Result<(i64, Vec<FileDb>), AppError> {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;

        let total: i64 = build_filtered_query(search_term.as_deref(), &filter_list)
            .count()
            .get_result(&mut conn)
            .map_err(|e| AppError::InternalServerError(format!("db count error: {}", e)))?;

        let mut query = build_filtered_query(search_term.as_deref(), &filter_list);
        for sort in &sort_list {
            if let Some(column) = parse_column(&sort.id) {
                query = apply_sort(query, column, sort.desc);
            }
        }

        let rows: Vec<FileDb> = query
            .limit(size)
            .offset(offset)
            .load(&mut conn)
            .map_err(|e| AppError::InternalServerError(format!("db query error: {}", e)))?;

        Ok((total, rows))
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))??;

    let total_pages = if total == 0 { 0 } else { (total + size - 1) / size };
    let content: Vec<schema::FileListResponse> = rows.iter().map(schema::FileListResponse::from).collect();

    let response = PaginatedResponse {
        total_elements: total,
        total_pages,
        content,
    };

    log::info!("Files listed: page={}, size={}", page, size);

    Ok(app_ok(&response))
}

fn build_filtered_query(search: Option<&str>, filters: &[crate::dto::request::filter_request::Filter]) -> FilesBaseQuery {
    use model::files;

    let mut query = files::table.into_boxed::<diesel::mysql::Mysql>();

    if let Some(term) = search.map(str::trim).filter(|t| !t.is_empty()) {
        if let Some(predicate) = search_predicate(term) {
            query = query.filter(predicate);
        }
    }

    for filter in filters {
        let Some(column) = parse_column(&filter.id) else {
            continue;
        };
        if let Some(predicate) = filter_predicate(column, &filter.match_mode, &filter.value) {
            query = query.filter(predicate);
        }
    }

    query
}

fn search_predicate(term: &str) -> Option<FilesPredicate> {
    use model::files;

    let like = format!("%{}%", term);
    let predicate = files::file_name
        .like(like.clone())
        .or(files::stored_name.like(like.clone()))
        .or(files::content_type.like(like.clone()))
        .or(files::file_type.like(like));
    Some(Box::new(predicate) as FilesPredicate)
}

fn filter_predicate(column: FileColumn, mode: &FilterMatchMode, value: &str) -> Option<FilesPredicate> {
    match column {
        FileColumn::Size => numeric_predicate(mode, value),
        FileColumn::CreatedAt => date_predicate(mode, value),
        FileColumn::UpdatedAt => date_updated_predicate(mode, value),
        _ => text_predicate(column, mode, value),
    }
}

fn text_predicate(column: FileColumn, mode: &FilterMatchMode, value: &str) -> Option<FilesPredicate> {
    use model::files;

    macro_rules! text_like {
        ($col:expr) => {
            match mode {
                FilterMatchMode::CONTAINS => Some(Box::new($col.like(format!("%{}%", value))) as FilesPredicate),
                FilterMatchMode::SW => Some(Box::new($col.like(format!("{}%", value))) as FilesPredicate),
                FilterMatchMode::EW => Some(Box::new($col.like(format!("%{}", value))) as FilesPredicate),
                FilterMatchMode::EQUALS => Some(Box::new($col.eq(value.to_owned())) as FilesPredicate),
                FilterMatchMode::NOT => Some(Box::new($col.ne(value.to_owned())) as FilesPredicate),
                _ => None,
            }
        };
    }

    match column {
        FileColumn::Id => text_like!(files::id),
        FileColumn::FileName => text_like!(files::file_name),
        FileColumn::StoredName => text_like!(files::stored_name),
        FileColumn::FilePath => text_like!(files::file_path),
        FileColumn::ContentType => text_like!(files::content_type),
        FileColumn::FileType => text_like!(files::file_type),
        _ => None,
    }
}

fn numeric_predicate(mode: &FilterMatchMode, value: &str) -> Option<FilesPredicate> {
    use model::files;

    match mode {
        FilterMatchMode::GT => parse_number(value).map(|n| Box::new(files::size.gt(n)) as FilesPredicate),
        FilterMatchMode::LT => parse_number(value).map(|n| Box::new(files::size.lt(n)) as FilesPredicate),
        FilterMatchMode::NOT => parse_number(value).map(|n| Box::new(files::size.ne(n)) as FilesPredicate),
        FilterMatchMode::BETWEEN => parse_range_number(value).map(|(lo, hi)| Box::new(files::size.between(lo, hi)) as FilesPredicate),
        _ => parse_number(value).map(|n| Box::new(files::size.eq(n)) as FilesPredicate),
    }
}

fn date_predicate(mode: &FilterMatchMode, value: &str) -> Option<FilesPredicate> {
    use model::files;

    match mode {
        FilterMatchMode::GT => parse_date(value).map(|d| Box::new(files::created_at.gt(d)) as FilesPredicate),
        FilterMatchMode::LT => parse_date(value).map(|d| Box::new(files::created_at.lt(d)) as FilesPredicate),
        FilterMatchMode::NOT => parse_date(value).map(|d| Box::new(files::created_at.ne(d)) as FilesPredicate),
        FilterMatchMode::BETWEEN => parse_range_date(value).map(|(lo, hi)| Box::new(files::created_at.between(lo, hi)) as FilesPredicate),
        _ => parse_date(value).map(|d| Box::new(files::created_at.eq(d)) as FilesPredicate),
    }
}

fn date_updated_predicate(mode: &FilterMatchMode, value: &str) -> Option<FilesPredicate> {
    use model::files;

    match mode {
        FilterMatchMode::GT => parse_date(value).map(|d| Box::new(files::updated_at.gt(d)) as FilesPredicate),
        FilterMatchMode::LT => parse_date(value).map(|d| Box::new(files::updated_at.lt(d)) as FilesPredicate),
        FilterMatchMode::NOT => parse_date(value).map(|d| Box::new(files::updated_at.ne(d)) as FilesPredicate),
        FilterMatchMode::BETWEEN => parse_range_date(value).map(|(lo, hi)| Box::new(files::updated_at.between(lo, hi)) as FilesPredicate),
        _ => parse_date(value).map(|d| Box::new(files::updated_at.eq(d)) as FilesPredicate),
    }
}

fn apply_sort(mut query: FilesBaseQuery, column: FileColumn, desc: bool) -> FilesBaseQuery {
    use model::files;

    match column {
        FileColumn::Id => {
            if desc {
                query = query.then_order_by(files::id.desc());
            } else {
                query = query.then_order_by(files::id.asc());
            }
        }
        FileColumn::FileName => {
            if desc {
                query = query.then_order_by(files::file_name.desc());
            } else {
                query = query.then_order_by(files::file_name.asc());
            }
        }
        FileColumn::StoredName => {
            if desc {
                query = query.then_order_by(files::stored_name.desc());
            } else {
                query = query.then_order_by(files::stored_name.asc());
            }
        }
        FileColumn::FilePath => {
            if desc {
                query = query.then_order_by(files::file_path.desc());
            } else {
                query = query.then_order_by(files::file_path.asc());
            }
        }
        FileColumn::ContentType => {
            if desc {
                query = query.then_order_by(files::content_type.desc());
            } else {
                query = query.then_order_by(files::content_type.asc());
            }
        }
        FileColumn::FileType => {
            if desc {
                query = query.then_order_by(files::file_type.desc());
            } else {
                query = query.then_order_by(files::file_type.asc());
            }
        }
        FileColumn::Size => {
            if desc {
                query = query.then_order_by(files::size.desc());
            } else {
                query = query.then_order_by(files::size.asc());
            }
        }
        FileColumn::CreatedAt => {
            if desc {
                query = query.then_order_by(files::created_at.desc());
            } else {
                query = query.then_order_by(files::created_at.asc());
            }
        }
        FileColumn::UpdatedAt => {
            if desc {
                query = query.then_order_by(files::updated_at.desc());
            } else {
                query = query.then_order_by(files::updated_at.asc());
            }
        }
    }

    query
}

#[derive(Clone, Copy, PartialEq)]
enum FileColumn {
    Id,
    FileName,
    StoredName,
    FilePath,
    ContentType,
    FileType,
    Size,
    CreatedAt,
    UpdatedAt,
}

fn parse_column(id: &str) -> Option<FileColumn> {
    let normalized = id.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "id" => Some(FileColumn::Id),
        "file_name" | "filename" => Some(FileColumn::FileName),
        "stored_name" | "storedname" => Some(FileColumn::StoredName),
        "file_path" | "filepath" => Some(FileColumn::FilePath),
        "content_type" | "contenttype" => Some(FileColumn::ContentType),
        "file_type" | "filetype" => Some(FileColumn::FileType),
        "size" => Some(FileColumn::Size),
        "created_at" | "createdat" => Some(FileColumn::CreatedAt),
        "updated_at" | "updatedat" => Some(FileColumn::UpdatedAt),
        _ => None,
    }
}

fn parse_number(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

fn parse_range_number(value: &str) -> Option<(i64, i64)> {
    let mut parts = value.split(',');
    let lo = parts.next()?.trim().parse::<i64>().ok()?;
    let hi = parts.next()?.trim().parse::<i64>().ok()?;
    Some((lo, hi))
}

fn parse_date(value: &str) -> Option<NaiveDateTime> {
    let value = value.trim();
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
}

fn parse_range_date(value: &str) -> Option<(NaiveDateTime, NaiveDateTime)> {
    let mut parts = value.split(',');
    let lo = parse_date(parts.next()?)?;
    let hi = parse_date(parts.next()?)?;
    Some((lo, hi))
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

fn app_ok<T: serde::Serialize>(data: &T) -> Response<AppBody> {
    app_response::ok(data)
}

async fn find_file(file_id: &str) -> Result<FileDb, AppError> {
    let file_id = file_id.to_string();
    tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        let mut conn = database::get_conn()?;
        model::files::table
            .find(&file_id)
            .select(FileDb::as_select())
            .first(&mut conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => {
                    AppError::NotFound(format!("file with id '{}' not found", file_id))
                }
                e => AppError::InternalServerError(format!("db query error: {}", e)),
            })
    })
    .await
    .map_err(|e| AppError::InternalServerError(format!("spawn_blocking error: {}", e)))?
}

enum Range {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

fn parse_range(header: Option<&str>, total: u64) -> Range {
    let Some(header) = header else {
        return Range::Full;
    };
    let Some(range) = header.trim().strip_prefix("bytes=") else {
        return Range::Full;
    };
    if range.contains(',') {
        return Range::Full;
    }
    let range = range.trim();

    if let Some((start_str, end_str)) = range.split_once('-') {
        let start = match start_str.trim().parse::<u64>() {
            Ok(v) => v,
            Err(_) => return Range::Full,
        };
        let end = if end_str.trim().is_empty() {
            total.saturating_sub(1)
        } else {
            match end_str.trim().parse::<u64>() {
                Ok(v) => v,
                Err(_) => return Range::Full,
            }
        };
        if total == 0 || start >= total {
            return Range::Unsatisfiable;
        }
        let end = end.min(total - 1);
        if start > end {
            return Range::Unsatisfiable;
        }
        return Range::Partial { start, end };
    }

    if let Some(suffix) = range.strip_prefix('-') {
        let n = match suffix.trim().parse::<u64>() {
            Ok(v) => v,
            Err(_) => return Range::Full,
        };
        if n == 0 {
            return Range::Unsatisfiable;
        }
        let start = if n >= total { 0 } else { total - n };
        return Range::Partial {
            start,
            end: total - 1,
        };
    }

    Range::Full
}

fn range_not_satisfiable(total: u64) -> Result<Response<AppBody>, AppError> {
    Ok(Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header("accept-ranges", "bytes")
        .header("content-range", format!("bytes */{}", total))
        .body(app_response::full_bytes(Bytes::new()))
        .map_err(|e| AppError::InternalServerError(e.to_string()))?)
}