use std::{str::FromStr, time::Duration};

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use serenity::{
    all::{
        AutoArchiveDuration, ChannelId, ChannelType, Colour, GuildId, Member, MessageId,
        ReactionType, Role, RoleId, ScheduledEventStatus, ScheduledEventType, Timestamp, UserId,
    },
    http::Http,
};
use tokio_retry::{
    Retry,
    strategy::{ExponentialBackoff, jitter},
};

pub fn ok(data: Value) -> Value {
    json!({ "ok": true, "data": data })
}

pub fn err(message: impl ToString) -> Value {
    json!({ "ok": false, "error": message.to_string() })
}

pub fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|_| json!({ "error": "serialization_failed" }))
}

pub fn parse_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => value.parse::<u64>().ok(),
        _ => None,
    }
}

pub fn parse_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|v| u32::try_from(v).ok()),
        Value::String(value) => value.parse::<u32>().ok(),
        _ => None,
    }
}

pub fn parse_u16(value: &Value) -> Option<u16> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|v| u16::try_from(v).ok()),
        Value::String(value) => value.parse::<u16>().ok(),
        _ => None,
    }
}

pub fn parse_u8(value: &Value) -> Option<u8> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|v| u8::try_from(v).ok()),
        Value::String(value) => value.parse::<u8>().ok(),
        _ => None,
    }
}

pub fn parse_bool(value: &Value) -> Option<bool> {
    value.as_bool()
}

pub fn parse_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.to_string()),
        Value::Number(number) => number.as_u64().map(|v| v.to_string()),
        _ => None,
    }
}

pub fn parse_u64_list(value: &Value) -> Option<Vec<u64>> {
    let items = value.as_array()?;
    let parsed = items.iter().filter_map(parse_u64).collect::<Vec<_>>();
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

pub fn get_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(parse_u64)
}

pub fn get_u32(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(parse_u32)
}

pub fn get_u16(args: &Value, key: &str) -> Option<u16> {
    args.get(key).and_then(parse_u16)
}

pub fn get_u8(args: &Value, key: &str) -> Option<u8> {
    args.get(key).and_then(parse_u8)
}

pub fn get_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(parse_bool)
}

pub fn get_string(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(parse_string)
}

pub fn get_u64_list(args: &Value, key: &str) -> Option<Vec<u64>> {
    args.get(key).and_then(parse_u64_list)
}

pub fn get_guild_id_default(args: &Value) -> Option<GuildId> {
    get_u64(args, "guild_id").map(GuildId::new)
}

pub fn get_channel_id(args: &Value, key: &str) -> Option<ChannelId> {
    get_u64(args, key).map(ChannelId::new)
}

pub fn get_user_id(args: &Value, key: &str) -> Option<UserId> {
    get_u64(args, key).map(UserId::new)
}

pub fn get_message_id(args: &Value, key: &str) -> Option<MessageId> {
    get_u64(args, key).map(MessageId::new)
}

pub fn parse_channel_type(value: &Value) -> Option<ChannelType> {
    let raw = value.as_str()?.trim().to_lowercase();
    match raw.as_str() {
        "text" | "guild_text" => Some(ChannelType::Text),
        "voice" | "guild_voice" => Some(ChannelType::Voice),
        "category" | "guild_category" => Some(ChannelType::Category),
        "news" | "announcement" | "guild_news" => Some(ChannelType::News),
        "stage" | "stage_voice" => Some(ChannelType::Stage),
        "forum" | "guild_forum" => Some(ChannelType::Forum),
        "public_thread" => Some(ChannelType::PublicThread),
        "private_thread" => Some(ChannelType::PrivateThread),
        "news_thread" => Some(ChannelType::NewsThread),
        _ => None,
    }
}

pub fn parse_thread_type(value: &Value) -> Option<ChannelType> {
    let raw = value.as_str()?.trim().to_lowercase();
    match raw.as_str() {
        "public" | "public_thread" => Some(ChannelType::PublicThread),
        "private" | "private_thread" => Some(ChannelType::PrivateThread),
        "news" | "news_thread" => Some(ChannelType::NewsThread),
        _ => None,
    }
}

pub fn parse_auto_archive_duration(value: &Value) -> Option<AutoArchiveDuration> {
    let minutes = parse_u64(value)?;
    match minutes {
        60 => Some(AutoArchiveDuration::OneHour),
        1440 => Some(AutoArchiveDuration::OneDay),
        4320 => Some(AutoArchiveDuration::ThreeDays),
        10080 => Some(AutoArchiveDuration::OneWeek),
        _ => None,
    }
}

pub fn parse_scheduled_event_type(value: &Value) -> Option<ScheduledEventType> {
    let raw = value.as_str()?.trim().to_lowercase();
    match raw.as_str() {
        "voice" => Some(ScheduledEventType::Voice),
        "stage" | "stage_instance" => Some(ScheduledEventType::StageInstance),
        "external" => Some(ScheduledEventType::External),
        _ => None,
    }
}

pub fn parse_scheduled_event_status(value: &Value) -> Option<ScheduledEventStatus> {
    let raw = value.as_str()?.trim().to_lowercase();
    match raw.as_str() {
        "scheduled" => Some(ScheduledEventStatus::Scheduled),
        "active" => Some(ScheduledEventStatus::Active),
        "completed" => Some(ScheduledEventStatus::Completed),
        "canceled" | "cancelled" => Some(ScheduledEventStatus::Canceled),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn parse_timestamp(value: &Value) -> Option<Timestamp> {
    value
        .as_str()
        .and_then(|value| Timestamp::parse(value).ok())
}

pub fn parse_colour(value: &Value) -> Option<Colour> {
    match value {
        Value::Number(number) => number.as_u64().map(|value| Colour::new(value as u32)),
        Value::String(value) => {
            let raw = value.trim_start_matches('#');
            u32::from_str_radix(raw, 16).ok().map(Colour::new)
        }
        _ => None,
    }
}

pub fn parse_reaction_type(value: &Value) -> Option<ReactionType> {
    parse_string(value).and_then(|value| ReactionType::from_str(&value).ok())
}

pub fn discord_retry_strategy() -> impl Iterator<Item = Duration> {
    ExponentialBackoff::from_millis(100)
        .max_delay(Duration::from_secs(10))
        .map(jitter)
        .take(5)
}

pub async fn retry_discord<F, Fut, T>(f: F) -> serenity::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = serenity::Result<T>>,
{
    Retry::spawn(discord_retry_strategy(), f).await
}

pub fn parse_relative_time(duration_str: &str) -> Option<Duration> {
    let duration_str = duration_str.trim().to_lowercase();
    if duration_str.is_empty() {
        return None;
    }

    let mut total_secs = 0;
    let mut current_num = String::new();

    for c in duration_str.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else {
            let num: u64 = current_num.parse().unwrap_or(0);
            match c {
                's' => total_secs += num,
                'm' => total_secs += num * 60,
                'h' => total_secs += num * 3600,
                'd' => total_secs += num * 86400,
                'w' => total_secs += num * 604800,
                _ => return None, // Invalid format
            }
            current_num.clear();
        }
    }

    if !current_num.is_empty() {
        // If it ends with just a number, assume seconds if the string was pure numbers, or error otherwise.
        if duration_str.chars().all(|c| c.is_ascii_digit()) {
            total_secs += current_num.parse::<u64>().unwrap_or(0);
        } else {
            return None;
        }
    }

    if total_secs == 0 {
        None
    } else {
        Some(Duration::from_secs(total_secs))
    }
}

pub fn resolve_relative_timestamp(duration_str: &str) -> Option<Timestamp> {
    if duration_str == "clear" {
        return None;
    }
    let duration = parse_relative_time(duration_str)?;
    let future_time = Utc::now() + duration;
    Timestamp::parse(&future_time.to_rfc3339()).ok()
}

pub fn snowflake_to_datetime(snowflake: u64) -> DateTime<Utc> {
    let unix_ms = ((snowflake >> 22) + 1_420_070_400_000) as i64;
    Utc.timestamp_millis_opt(unix_ms)
        .single()
        .unwrap_or_else(Utc::now)
}

pub async fn fetch_guild_members(
    http: &Http,
    guild_id: GuildId,
    limit: u64,
) -> serenity::Result<Vec<Member>> {
    let mut all_members = Vec::new();
    let mut after = None;

    while (all_members.len() as u64) < limit {
        let batch_limit = (limit - all_members.len() as u64).min(1000);
        let mut members = guild_id.members(http, Some(batch_limit), after).await?;
        if members.is_empty() {
            break;
        }

        let fetched_count = members.len();
        after = members.last().map(|member| member.user.id);
        all_members.append(&mut members);

        if fetched_count < batch_limit as usize {
            break;
        }
    }

    Ok(all_members)
}

pub async fn resolve_user_id(http: &Http, guild_id: GuildId, query: &str) -> Option<UserId> {
    let cleaned = query
        .trim()
        .trim_start_matches("<@")
        .trim_start_matches('!')
        .trim_end_matches('>');
    if let Some(id) = parse_u64(&Value::String(cleaned.to_string())) {
        return Some(UserId::new(id));
    }

    let members = fetch_guild_members(http, guild_id, 5_000).await.ok()?;
    let query_lower = query.trim().to_lowercase();
    let mut matches = members
        .into_iter()
        .filter(|member| {
            member.user.name.to_lowercase().contains(&query_lower)
                || member
                    .nick
                    .as_ref()
                    .is_some_and(|nick| nick.to_lowercase().contains(&query_lower))
                || member
                    .user
                    .global_name
                    .as_ref()
                    .is_some_and(|global| global.to_lowercase().contains(&query_lower))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| left.user.name.cmp(&right.user.name));
    matches.first().map(|member| member.user.id)
}

pub async fn resolve_role_id(http: &Http, guild_id: GuildId, query: &str) -> Option<RoleId> {
    let cleaned = query.trim().trim_start_matches("<@&").trim_end_matches('>');
    if let Some(id) = parse_u64(&Value::String(cleaned.to_string())) {
        return Some(RoleId::new(id));
    }

    let roles = guild_id.roles(http).await.ok()?;
    let query_lower = query.trim().to_lowercase();
    let mut matches = roles
        .values()
        .filter(|role: &&Role| role.name.to_lowercase().contains(&query_lower))
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| left.name.cmp(&right.name));
    matches.first().map(|role| role.id)
}

pub async fn resolve_role_ids(http: &Http, guild_id: GuildId, queries: &[String]) -> Vec<RoleId> {
    let mut ids = Vec::new();
    for query in queries {
        if let Some(id) = resolve_role_id(http, guild_id, query).await {
            ids.push(id);
        }
    }
    ids
}

/// Macro to generate a standard `pub fn new(http: Arc<Http>) -> Self` constructor
/// for tool structs that have a single `http: Arc<Http>` field.
///
/// Usage:
/// ```ignore
/// impl_new!(SendMessageTool, CreatePoll, ListRoles);
/// ```
#[macro_export]
macro_rules! impl_new {
    ($($ty:ty),* $(,)?) => {
        $(
            impl $ty {
                #[allow(dead_code)]
                pub fn new(http: Arc<serenity::http::Http>) -> Self {
                    Self { http }
                }
            }
        )*
    };
}
