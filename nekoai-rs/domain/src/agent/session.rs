use serenity::all::{ChannelId, GuildId};

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize)]
pub enum SessionKind {
    GuildChannel,
    Thread,
    DirectMessage,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize)]
pub struct SessionKey {
    pub guild_id: Option<GuildId>,
    pub channel_id: ChannelId,
    pub thread_id: Option<ChannelId>,
    pub kind: SessionKind,
}
