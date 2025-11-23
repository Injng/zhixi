use rocket_db_pools::{sqlx, Database};

#[derive(Database)]
#[database("sqlite_logs")]
pub struct Db(sqlx::SqlitePool);
