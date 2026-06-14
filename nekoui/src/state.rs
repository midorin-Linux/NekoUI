use crate::repositories::sqlite::SqliteRepository;

#[derive(Clone)]
pub struct ServerState {
    pub sqlite_repo: SqliteRepository,
}
