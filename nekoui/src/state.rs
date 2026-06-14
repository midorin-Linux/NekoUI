use std::sync::Arc;

use crate::services::Services;

#[derive(Clone)]
pub struct ServerState {
    pub services: Arc<Services>,
}
