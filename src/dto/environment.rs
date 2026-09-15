use std::fs;

use serde::Deserialize;

use super::enumerator::database_type::DatabaseType;

#[derive(Clone, Deserialize, Debug)]
pub struct Environment {
    pub auth_salt: String,

    pub database_filemanagement_type: DatabaseType,
    pub database_filemanagement_username: String,
    pub database_filemanagement_password: String,
    pub database_filemanagement_host: String,
    pub database_filemanagement_port: u16,
    pub database_filemanagement_dbname: String,
    pub database_filemanagement_path: String,
    pub database_filemanagement_max_pool: u32,
    pub database_filemanagement_min_pool: u32,

    pub minio_endpoint: String,
    pub minio_access_key: String,
    pub minio_secret_key: String,
    pub minio_bucket: String,
    pub minio_region: String,

    pub jwt_expiration: i64,
    pub jwt_key: String,

    pub redis_host: String,
    pub redis_port: u16,
    pub redis_password: String,
    pub redis_timeout: u64,
    pub redis_ssl: bool,

    pub redis_cache_enable: bool,

    pub rust_backtrace: u8,
    pub rust_log: String,

    pub logger_level: u8,

    pub rate_limiter_max_connection: usize,
    pub rate_limiter_time_reset_connection: u64,

    pub server_host: String,
    pub server_port: u16,
    pub server_thread: usize,
    pub server_tls: bool,
    pub server_tls_cert_file: String,
    pub server_tls_key_file: String,

    pub session_key: String,
    pub session_name: String,
    pub session_secure: bool,
    pub session_timeout: i64,
}

impl Environment {
    pub fn from_file(path: &'static str) -> Self {
        let config = fs::read_to_string(path).unwrap();
        serde_json::from_str(&config).unwrap()
    }

    pub fn get_server_url(&self) -> String {
        format!("{0}:{1}", self.server_host, self.server_port)
    }

    pub fn get_redis_url(&self) -> String {
        format!(
            "redis://{0}:{1}@{2}:{3}",
            "default".to_string(),
            self.redis_password,
            self.redis_host,
            self.redis_port,
        )
    }

    pub fn get_database_filemanagement_url(&self) -> String {
        match self.database_filemanagement_type {
            DatabaseType::MariaDB | DatabaseType::MySQL => format!(
                "mysql://{0}:{1}@{2}:{3}/{4}",
                self.database_filemanagement_username,
                self.database_filemanagement_password,
                self.database_filemanagement_host,
                self.database_filemanagement_port,
                self.database_filemanagement_dbname
            ),
            DatabaseType::Sqlite => self.database_filemanagement_path.clone(),
            DatabaseType::Postgres => format!(
                "postgresql://{0}:{1}@{2}:{3}/{4}",
                self.database_filemanagement_username,
                self.database_filemanagement_password,
                self.database_filemanagement_host,
                self.database_filemanagement_port,
                self.database_filemanagement_dbname
            ),
        }
    }
}
