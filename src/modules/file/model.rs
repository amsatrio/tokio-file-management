use chrono::NaiveDateTime;
use diesel::prelude::*;

diesel::table! {
    files (id) {
        id -> Varchar,
        file_name -> Varchar,
        stored_name -> Varchar,
        file_path -> Varchar,
        content_type -> Varchar,
        file_type -> Varchar,
        size -> Bigint,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = files)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct FileDb {
    pub id: String,
    pub file_name: String,
    pub stored_name: String,
    pub file_path: String,
    pub content_type: String,
    pub file_type: String,
    pub size: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = files)]
pub struct NewFile {
    pub id: String,
    pub file_name: String,
    pub stored_name: String,
    pub file_path: String,
    pub content_type: String,
    pub file_type: String,
    pub size: i64,
}