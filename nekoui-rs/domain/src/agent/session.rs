use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionKey {
    /// チャネルまたは会話の UUID（クライアントが生成）
    pub conversation_id: Uuid,
    /// ユーザー識別子（オプション：認証なしも許容）
    pub user_id: Option<String>,
}

impl SessionKey {
    pub fn new(conversation_id: Uuid, user_id: Option<String>) -> Self {
        Self {
            conversation_id,
            user_id,
        }
    }

    /// 匿名セッション
    pub fn anonymous(conversation_id: Uuid) -> Self {
        Self {
            conversation_id,
            user_id: None,
        }
    }
}
