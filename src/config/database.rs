use diesel::mysql::MysqlConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lazy_static::lazy_static;

use crate::dto::environment::Environment;
use crate::modules::hello_world::schema::AppError;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub type DbConnection = MysqlConnection;
pub type DbPool = Pool<ConnectionManager<DbConnection>>;

lazy_static! {
    pub static ref POOL: DbPool = build_pool();
}

fn build_pool() -> DbPool {
    let config: &Environment = &crate::config::environment::CONFIG;

    let manager = ConnectionManager::<DbConnection>::new(config.get_database_filemanagement_url());
    Pool::builder()
        .max_size(config.database_filemanagement_max_pool)
        .min_idle(Some(config.database_filemanagement_min_pool))
        .build(manager)
        .expect("failed to create database pool")
}

pub fn run_migrations() {
    let mut conn = POOL
        .get()
        .expect("failed to get database connection for migrations");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("failed to run database migrations");
    log::info!("Database migrations applied");
}

pub fn get_conn() -> Result<PooledConnection<ConnectionManager<DbConnection>>, AppError> {
    POOL.get()
        .map_err(|e| AppError::InternalServerError(format!("failed to get db connection: {}", e)))
}