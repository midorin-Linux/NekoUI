use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct SessionKey {
    pub conversation_id: Uuid,
    pub user_id: Option<String>,
}

impl SessionKey {
    pub fn new(conversation_id: Uuid, user_id: Option<String>) -> Self {
        Self {
            conversation_id,
            user_id,
        }
    }
}
