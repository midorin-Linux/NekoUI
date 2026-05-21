use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use chrono::{
    Duration as ChronoDuration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
    Utc,
};
use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use serenity::{
    all::{
        ChannelId, ChannelType, CreateScheduledEvent, EditScheduledEvent, GuildChannel, GuildId,
        ScheduledEvent, ScheduledEventId, ScheduledEventStatus, ScheduledEventType, Timestamp,
    },
    http::{Http, UserPagination},
};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, get_bool, get_channel_id, get_guild_id_default, get_string, get_u64, ok,
            parse_relative_time, parse_scheduled_event_status, parse_scheduled_event_type,
            retry_discord, to_value,
        },
    },
    impl_new,
};

const DEFAULT_EXTERNAL_EVENT_DURATION_MINUTES: i64 = 60;
const SEARCH_LIMIT_DEFAULT: usize = 20;
const SEARCH_LIMIT_MAX: usize = 100;

#[derive(Clone)]
struct ScheduledEventMatch {
    event: ScheduledEvent,
    score: Option<u32>,
    match_reasons: Vec<String>,
}

#[derive(Clone)]
struct ChannelMatch {
    channel: GuildChannel,
    score: u32,
    match_reasons: Vec<String>,
}

pub struct CreateScheduledEventTool {
    http: Arc<Http>,
}

pub struct ListEvents {
    http: Arc<Http>,
}

pub struct UpdateOrCancelEvent {
    http: Arc<Http>,
}

pub struct GetEventSubscribers {
    http: Arc<Http>,
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn get_trimmed_string(args: &Value, key: &str) -> Option<String> {
    trim_optional(get_string(args, key))
}

fn event_kind_label(kind: ScheduledEventType) -> &'static str {
    match kind {
        ScheduledEventType::Voice => "voice",
        ScheduledEventType::StageInstance => "stage",
        ScheduledEventType::External => "external",
        _ => "unknown",
    }
}

fn event_status_label(status: ScheduledEventStatus) -> &'static str {
    match status {
        ScheduledEventStatus::Scheduled => "scheduled",
        ScheduledEventStatus::Active => "active",
        ScheduledEventStatus::Completed => "completed",
        ScheduledEventStatus::Canceled => "canceled",
        _ => "unknown",
    }
}

fn channel_kind_label(kind: ChannelType) -> &'static str {
    match kind {
        ChannelType::Voice => "voice",
        ChannelType::Stage => "stage",
        ChannelType::Text => "text",
        ChannelType::News => "news",
        ChannelType::Category => "category",
        ChannelType::Forum => "forum",
        ChannelType::PublicThread => "public_thread",
        ChannelType::PrivateThread => "private_thread",
        ChannelType::NewsThread => "news_thread",
        _ => "unknown",
    }
}

fn event_summary_value(event: &ScheduledEvent) -> Value {
    json!({
        "id": event.id.get(),
        "guild_id": event.guild_id.get(),
        "channel_id": event.channel_id.map(|value| value.get()),
        "name": event.name.clone(),
        "description": event.description.clone(),
        "start_time": event.start_time.to_string(),
        "end_time": event.end_time.map(|value| value.to_string()),
        "kind": event_kind_label(event.kind),
        "status": event_status_label(event.status),
        "location": event.metadata.as_ref().and_then(|metadata| metadata.location.clone()),
        "user_count": event.user_count,
    })
}

fn channel_summary_value(channel: &GuildChannel) -> Value {
    json!({
        "id": channel.id.get(),
        "name": channel.name.clone(),
        "kind": channel_kind_label(channel.kind),
    })
}

fn parse_event_kind(raw: &str) -> Option<ScheduledEventType> {
    parse_scheduled_event_type(&Value::String(raw.to_string()))
}

fn parse_event_status(raw: &str) -> Option<ScheduledEventStatus> {
    parse_scheduled_event_status(&Value::String(raw.to_string()))
}

fn get_optional_event_kind(args: &Value, key: &str) -> Result<Option<ScheduledEventType>, String> {
    match get_trimmed_string(args, key) {
        Some(raw) => parse_event_kind(&raw)
            .map(Some)
            .ok_or_else(|| format!("Invalid {key}: {raw}. Use voice, stage, or external.")),
        None => Ok(None),
    }
}

fn get_optional_event_status(
    args: &Value,
    key: &str,
) -> Result<Option<ScheduledEventStatus>, String> {
    match get_trimmed_string(args, key) {
        Some(raw) => parse_event_status(&raw).map(Some).ok_or_else(|| {
            format!("Invalid {key}: {raw}. Use scheduled, active, completed, or canceled.")
        }),
        None => Ok(None),
    }
}

fn parse_channel_reference_id(reference: &str) -> Option<u64> {
    let trimmed = reference.trim();
    let trimmed = trimmed
        .strip_prefix("<#")
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix('#').unwrap_or(trimmed);
    trimmed.parse::<u64>().ok()
}

fn channel_kind_for_event_kind(kind: ScheduledEventType) -> Option<ChannelType> {
    match kind {
        ScheduledEventType::Voice => Some(ChannelType::Voice),
        ScheduledEventType::StageInstance => Some(ChannelType::Stage),
        _ => None,
    }
}

fn event_kind_for_channel_kind(kind: ChannelType) -> Option<ScheduledEventType> {
    match kind {
        ChannelType::Voice => Some(ScheduledEventType::Voice),
        ChannelType::Stage => Some(ScheduledEventType::StageInstance),
        _ => None,
    }
}

fn channel_matches_event_kind(channel: &GuildChannel, kind: ScheduledEventType) -> bool {
    channel_kind_for_event_kind(kind).is_some_and(|expected| channel.kind == expected)
}

fn add_seconds_to_timestamp(start_time: Timestamp, seconds: i64) -> Option<Timestamp> {
    let unix = start_time.unix_timestamp().checked_add(seconds)?;
    Timestamp::from_unix_timestamp(unix).ok()
}

fn apply_duration_minutes(start_time: Timestamp, minutes: i64) -> Option<Timestamp> {
    let seconds = minutes.checked_mul(60)?;
    add_seconds_to_timestamp(start_time, seconds)
}

fn event_duration_seconds(event: &ScheduledEvent) -> Option<i64> {
    let end_time = event.end_time?;
    let duration = end_time
        .unix_timestamp()
        .checked_sub(event.start_time.unix_timestamp())?;
    (duration > 0).then_some(duration)
}

fn local_naive_to_timestamp(naive: NaiveDateTime) -> Option<Timestamp> {
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(datetime) => Timestamp::from_unix_timestamp(datetime.timestamp()).ok(),
        LocalResult::Ambiguous(datetime, _) => {
            Timestamp::from_unix_timestamp(datetime.timestamp()).ok()
        }
        LocalResult::None => None,
    }
}

fn parse_local_time_of_day(value: &str) -> Option<NaiveTime> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let upper = trimmed.to_ascii_uppercase();
    for format in [
        "%H:%M",
        "%H:%M:%S",
        "%I%p",
        "%I %p",
        "%I:%M%p",
        "%I:%M %p",
        "%I:%M:%S%p",
        "%I:%M:%S %p",
    ] {
        if let Ok(time) = NaiveTime::parse_from_str(&upper, format) {
            return Some(time);
        }
    }

    None
}

fn parse_local_date(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, format) {
            return Some(date);
        }
    }

    None
}

fn parse_local_timestamp(value: &str) -> Option<Timestamp> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    for format in [
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y/%m/%dT%H:%M",
        "%Y/%m/%dT%H:%M:%S",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, format) {
            return local_naive_to_timestamp(naive);
        }
    }

    if let Some((date_part, time_part)) = trimmed.split_once(' ') {
        let date = parse_local_date(date_part)?;
        let time = parse_local_time_of_day(time_part)?;
        return local_naive_to_timestamp(date.and_time(time));
    }

    parse_local_date(trimmed)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .and_then(local_naive_to_timestamp)
}

fn parse_smart_timestamp(value: &str) -> Option<Timestamp> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(timestamp) = Timestamp::parse(trimmed) {
        return Some(timestamp);
    }

    if trimmed.eq_ignore_ascii_case("now") {
        return Timestamp::from_unix_timestamp(Utc::now().timestamp()).ok();
    }

    let relative_input = trimmed.strip_prefix("in ").unwrap_or(trimmed);
    let compact_relative = relative_input.replace(' ', "");
    if let Some(duration) = parse_relative_time(&compact_relative) {
        let chrono_duration = ChronoDuration::from_std(duration).ok()?;
        let future = Utc::now() + chrono_duration;
        return Timestamp::from_unix_timestamp(future.timestamp()).ok();
    }

    let lower = trimmed.to_ascii_lowercase();
    for prefix in ["today ", "tomorrow "] {
        if lower.starts_with(prefix) {
            let date = if prefix == "today " {
                Local::now().date_naive()
            } else {
                Local::now().date_naive() + ChronoDuration::days(1)
            };
            let rest = trimmed[prefix.len() ..].trim();
            let time = parse_local_time_of_day(rest)?;
            return local_naive_to_timestamp(date.and_time(time));
        }
    }

    parse_local_timestamp(trimmed)
}

fn score_text_field(
    value: &str,
    query: &str,
    exact: u32,
    prefix: u32,
    contains: u32,
    label: &str,
) -> Option<(u32, String)> {
    let value = normalize_text(value);
    if value == query {
        Some((exact, format!("{label} exact match")))
    } else if value.starts_with(query) {
        Some((prefix, format!("{label} prefix match")))
    } else if value.contains(query) {
        Some((contains, format!("{label} contains match")))
    } else {
        None
    }
}

fn score_scheduled_event(event: &ScheduledEvent, query: &str) -> Option<(u32, Vec<String>)> {
    let query = normalize_text(query);
    if query.is_empty() {
        return None;
    }

    let mut best_score = 0u32;
    let mut reasons = Vec::new();

    let mut consider = |match_value: Option<(u32, String)>| {
        if let Some((score, reason)) = match_value {
            if score > best_score {
                best_score = score;
                reasons.clear();
                reasons.push(reason);
            } else if score == best_score {
                reasons.push(reason);
            }
        }
    };

    consider(score_text_field(
        &event.name,
        &query,
        1000,
        900,
        800,
        "name",
    ));
    consider(score_text_field(
        &event.id.get().to_string(),
        &query,
        950,
        900,
        850,
        "id",
    ));
    consider(
        event
            .description
            .as_deref()
            .and_then(|value| score_text_field(value, &query, 700, 650, 600, "description")),
    );
    consider(
        event
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.location.as_deref())
            .and_then(|value| score_text_field(value, &query, 700, 650, 600, "location")),
    );
    consider(score_text_field(
        &event.start_time.to_string(),
        &query,
        500,
        450,
        400,
        "start_time",
    ));
    consider(
        event.end_time.as_ref().and_then(|value| {
            score_text_field(&value.to_string(), &query, 500, 450, 400, "end_time")
        }),
    );
    consider(score_text_field(
        &event
            .channel_id
            .map(|value| value.get().to_string())
            .unwrap_or_default(),
        &query,
        350,
        300,
        250,
        "channel_id",
    ));
    consider(score_text_field(
        event_kind_label(event.kind),
        &query,
        300,
        250,
        200,
        "kind",
    ));
    consider(score_text_field(
        event_status_label(event.status),
        &query,
        300,
        250,
        200,
        "status",
    ));

    (best_score > 0).then_some((best_score, reasons))
}

fn search_scheduled_event_matches(
    events: &[ScheduledEvent],
    query: Option<&str>,
    status_filter: Option<ScheduledEventStatus>,
    kind_filter: Option<ScheduledEventType>,
) -> Vec<ScheduledEventMatch> {
    let mut matches = Vec::new();

    for event in events {
        if let Some(status_filter) = status_filter
            && event.status != status_filter
        {
            continue;
        }

        if let Some(kind_filter) = kind_filter
            && event.kind != kind_filter
        {
            continue;
        }

        if let Some(query) = query {
            if let Some((score, match_reasons)) = score_scheduled_event(event, query) {
                matches.push(ScheduledEventMatch {
                    event: event.clone(),
                    score: Some(score),
                    match_reasons,
                });
            }
        } else {
            matches.push(ScheduledEventMatch {
                event: event.clone(),
                score: None,
                match_reasons: Vec::new(),
            });
        }
    }

    if query.is_some() {
        matches.sort_by(|left, right| {
            right
                .score
                .unwrap_or(0)
                .cmp(&left.score.unwrap_or(0))
                .then(left.event.start_time.cmp(&right.event.start_time))
                .then(left.event.name.cmp(&right.event.name))
        });
    } else {
        matches.sort_by(|left, right| {
            left.event
                .start_time
                .cmp(&right.event.start_time)
                .then(left.event.name.cmp(&right.event.name))
        });
    }

    matches
}

fn format_scheduled_event_candidates(matches: &[ScheduledEventMatch]) -> String {
    let mut lines = Vec::new();

    for (index, match_item) in matches.iter().take(5).enumerate() {
        lines.push(format!(
            "{}. {} (id: {}, start: {}, kind: {}, status: {}, score: {}, reasons: {})",
            index + 1,
            match_item.event.name.clone(),
            match_item.event.id.get(),
            match_item.event.start_time,
            event_kind_label(match_item.event.kind),
            event_status_label(match_item.event.status),
            match_item.score.unwrap_or(0),
            if match_item.match_reasons.is_empty() {
                "none".to_string()
            } else {
                match_item.match_reasons.join(", ")
            }
        ));
    }

    if matches.len() > 5 {
        lines.push(format!("... and {} more", matches.len() - 5));
    }

    lines.join("\n")
}

fn format_channel_candidates(matches: &[ChannelMatch]) -> String {
    let mut lines = Vec::new();

    for (index, match_item) in matches.iter().take(5).enumerate() {
        lines.push(format!(
            "{}. {} (id: {}, kind: {}, score: {}, reasons: {})",
            index + 1,
            match_item.channel.name,
            match_item.channel.id.get(),
            channel_kind_label(match_item.channel.kind),
            match_item.score,
            if match_item.match_reasons.is_empty() {
                "none".to_string()
            } else {
                match_item.match_reasons.join(", ")
            }
        ));
    }

    if matches.len() > 5 {
        lines.push(format!("... and {} more", matches.len() - 5));
    }

    lines.join("\n")
}

fn score_channel_match(channel: &GuildChannel, query: &str) -> Option<ChannelMatch> {
    let query = normalize_text(query);
    if query.is_empty() {
        return None;
    }

    let mut best_score = 0u32;
    let mut reasons = Vec::new();

    let mut consider = |match_value: Option<(u32, String)>| {
        if let Some((score, reason)) = match_value {
            if score > best_score {
                best_score = score;
                reasons.clear();
                reasons.push(reason);
            } else if score == best_score {
                reasons.push(reason);
            }
        }
    };

    consider(score_text_field(
        &channel.name,
        &query,
        1000,
        900,
        800,
        "name",
    ));
    consider(score_text_field(
        &channel.id.get().to_string(),
        &query,
        900,
        850,
        800,
        "id",
    ));

    (best_score > 0).then_some(ChannelMatch {
        channel: channel.clone(),
        score: best_score,
        match_reasons: reasons,
    })
}

async fn fetch_guild_scheduled_events(
    http: &Arc<Http>,
    guild_id: GuildId,
    with_user_count: bool,
) -> Result<Vec<ScheduledEvent>, String> {
    let http = http.clone();
    retry_discord(|| {
        let http = http.clone();
        async move { guild_id.scheduled_events(&http, with_user_count).await }
    })
    .await
    .map_err(|error| format!("Failed to fetch scheduled events: {error}"))
}

async fn fetch_guild_channels(
    http: &Arc<Http>,
    guild_id: GuildId,
) -> Result<HashMap<ChannelId, GuildChannel>, String> {
    let http = http.clone();
    retry_discord(|| {
        let http = http.clone();
        async move { guild_id.channels(&http).await }
    })
    .await
    .map_err(|error| format!("Failed to fetch channels: {error}"))
}

async fn resolve_scheduled_event_target(
    http: &Arc<Http>,
    guild_id: GuildId,
    target: &str,
) -> Result<ScheduledEvent, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("target_event is required".to_string());
    }

    let direct_fetch_error = if let Ok(event_id) = target.parse::<u64>() {
        match retry_discord(|| {
            let http = http.clone();
            async move {
                guild_id
                    .scheduled_event(&http, ScheduledEventId::new(event_id), false)
                    .await
            }
        })
        .await
        {
            Ok(event) => return Ok(event),
            Err(error) => Some(format!("{error}")),
        }
    } else {
        None
    };

    let events = fetch_guild_scheduled_events(http, guild_id, false).await?;
    let matches = search_scheduled_event_matches(&events, Some(target), None, None);

    if matches.is_empty() {
        return Err(match direct_fetch_error {
            Some(error) => format!("Could not resolve scheduled event '{target}': {error}"),
            None => format!("Could not resolve scheduled event: {target}"),
        });
    }

    let best_score = matches[0].score.unwrap_or(0);
    let tied = matches
        .iter()
        .take_while(|match_item| match_item.score.unwrap_or(0) == best_score)
        .count();

    if tied == 1 {
        return Ok(matches[0].event.clone());
    }

    Err(format!(
        "Multiple scheduled events match '{target}':\n{}",
        format_scheduled_event_candidates(&matches)
    ))
}

async fn resolve_scheduled_event_channel(
    http: &Arc<Http>,
    guild_id: GuildId,
    direct_channel_id: Option<ChannelId>,
    channel_query: Option<&str>,
    desired_channel_kind: Option<ChannelType>,
) -> Result<GuildChannel, String> {
    if let Some(channel_id) = direct_channel_id {
        let channels = fetch_guild_channels(http, guild_id).await?;
        let channel = channels
            .get(&channel_id)
            .cloned()
            .ok_or_else(|| format!("Could not resolve channel id: {}", channel_id.get()))?;

        if let Some(expected_kind) = desired_channel_kind
            && channel.kind != expected_kind
        {
            return Err(format!(
                "Channel {} is not a {} channel",
                channel.name,
                channel_kind_label(expected_kind)
            ));
        }

        return Ok(channel);
    }

    let Some(raw_query) = channel_query
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("channel_query is required".to_string());
    };

    let channels = fetch_guild_channels(http, guild_id).await?;

    if let Some(channel_id) = parse_channel_reference_id(raw_query)
        && let Some(channel) = channels.get(&ChannelId::new(channel_id)).cloned()
    {
        if let Some(expected_kind) = desired_channel_kind
            && channel.kind != expected_kind
        {
            return Err(format!(
                "Channel {} is not a {} channel",
                channel.name,
                channel_kind_label(expected_kind)
            ));
        }

        return Ok(channel);
    }

    let query = normalize_text(raw_query)
        .trim_start_matches('#')
        .trim()
        .to_string();
    let mut matches = channels
        .values()
        .filter(|channel| {
            desired_channel_kind.is_none_or(|expected_kind| channel.kind == expected_kind)
        })
        .filter_map(|channel| score_channel_match(channel, &query))
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.channel.name.cmp(&right.channel.name))
    });

    if matches.is_empty() {
        return Err(format!("Could not resolve channel: {raw_query}"));
    }

    let best_score = matches[0].score;
    let tied = matches
        .iter()
        .take_while(|match_item| match_item.score == best_score)
        .count();

    if tied == 1 {
        return Ok(matches[0].channel.clone());
    }

    Err(format!(
        "Multiple channels match '{raw_query}':\n{}",
        format_channel_candidates(&matches)
    ))
}

fn parse_duration_minutes(args: &Value, key: &str) -> Result<Option<i64>, String> {
    match get_u64(args, key) {
        Some(value) => {
            let minutes = i64::try_from(value).map_err(|_| format!("{key} is too large"))?;
            if minutes <= 0 {
                Err(format!("{key} must be greater than 0"))
            } else {
                Ok(Some(minutes))
            }
        }
        None => Ok(None),
    }
}

fn determine_create_end_time(
    start_time: Timestamp,
    duration_minutes: Option<i64>,
    explicit_end_time: Option<Timestamp>,
    final_kind: ScheduledEventType,
) -> Option<Timestamp> {
    if let Some(end_time) = explicit_end_time {
        Some(end_time)
    } else if let Some(minutes) = duration_minutes {
        apply_duration_minutes(start_time, minutes)
    } else if final_kind == ScheduledEventType::External {
        apply_duration_minutes(start_time, DEFAULT_EXTERNAL_EVENT_DURATION_MINUTES)
    } else {
        None
    }
}

fn determine_update_end_time(
    start_time: Timestamp,
    current_event: &ScheduledEvent,
    explicit_start_changed: bool,
    duration_minutes: Option<i64>,
    explicit_end_time: Option<Timestamp>,
    final_kind: ScheduledEventType,
) -> Option<Timestamp> {
    if let Some(end_time) = explicit_end_time {
        return Some(end_time);
    }

    if let Some(minutes) = duration_minutes {
        return apply_duration_minutes(start_time, minutes);
    }

    if explicit_start_changed && let Some(duration_seconds) = event_duration_seconds(current_event)
    {
        return add_seconds_to_timestamp(start_time, duration_seconds);
    }

    if final_kind == ScheduledEventType::External && current_event.end_time.is_none() {
        return apply_duration_minutes(start_time, DEFAULT_EXTERNAL_EVENT_DURATION_MINUTES);
    }

    None
}

fn event_match_to_value(match_item: &ScheduledEventMatch) -> Value {
    json!({
        "score": match_item.score,
        "match_reasons": match_item.match_reasons,
        "event": event_summary_value(&match_item.event),
    })
}

impl Tool for CreateScheduledEventTool {
    const NAME: &'static str = "create_scheduled_event";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Create a scheduled event using higher-level inputs. The tool accepts RFC3339 ",
                "timestamps, local timestamps like YYYY-MM-DD HH:MM, relative times like in 2h, ",
                "and date shortcuts like today 20:00 or tomorrow 20:00. Voice and stage events ",
                "resolve the channel by id or name; external events use a location and default to ",
                "a one-hour duration when no end time is given."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild id." },
                    "name": { "type": "string", "description": "Event name." },
                    "start_time": { "type": "string", "description": "Start time. Accepted forms: RFC3339, YYYY-MM-DD HH:MM, today 20:00, tomorrow 20:00, in 2h, etc." },
                    "kind": { "type": "string", "enum": ["voice", "stage", "external"], "description": "Optional event kind. If omitted, voice/stage is inferred from the channel and external is inferred from location." },
                    "channel_id": { "type": "integer", "description": "Voice or stage channel id." },
                    "channel_query": { "type": "string", "description": "Voice or stage channel name, mention, or id." },
                    "duration_minutes": { "type": "integer", "description": "Duration in minutes. Used to derive end_time." },
                    "end_time": { "type": "string", "description": "Optional end time. Same formats as start_time." },
                    "description": { "type": "string", "description": "Optional description." },
                    "location": { "type": "string", "description": "Location for external events." }
                },
                "required": ["guild_id", "name", "start_time"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let Some(name) = get_trimmed_string(&args, "name") else {
            return Ok(err("name is required"));
        };
        let Some(start_time_input) = get_trimmed_string(&args, "start_time") else {
            return Ok(err("start_time is required"));
        };
        let Some(start_time) = parse_smart_timestamp(&start_time_input) else {
            return Ok(err(format!(
                "Could not parse start_time: {start_time_input}"
            )));
        };
        let description = get_trimmed_string(&args, "description");
        let location = get_trimmed_string(&args, "location");
        let explicit_kind = match get_optional_event_kind(&args, "kind") {
            Ok(value) => value,
            Err(error) => return Ok(err(error)),
        };
        let duration_minutes = match parse_duration_minutes(&args, "duration_minutes") {
            Ok(value) => value,
            Err(error) => return Ok(err(error)),
        };
        let explicit_end_time = match get_trimmed_string(&args, "end_time") {
            Some(raw) => match parse_smart_timestamp(&raw) {
                Some(timestamp) => Some(timestamp),
                None => return Ok(err(format!("Could not parse end_time: {raw}"))),
            },
            None => None,
        };

        if let Some(end_time) = explicit_end_time
            && end_time.unix_timestamp() <= start_time.unix_timestamp()
        {
            return Ok(err("end_time must be after start_time"));
        }

        let channel_id = get_channel_id(&args, "channel_id");
        let channel_query = get_trimmed_string(&args, "channel_query");

        let (final_kind, resolved_channel) = if let Some(kind) = explicit_kind {
            match kind {
                ScheduledEventType::External => {
                    if location.is_none() {
                        return Ok(err("location is required for external scheduled events"));
                    }
                    (kind, None)
                }
                ScheduledEventType::Voice | ScheduledEventType::StageInstance => {
                    let desired_channel_kind = channel_kind_for_event_kind(kind);
                    let channel = match resolve_scheduled_event_channel(
                        &self.http,
                        guild_id,
                        channel_id,
                        channel_query.as_deref(),
                        desired_channel_kind,
                    )
                    .await
                    {
                        Ok(channel) => channel,
                        Err(error) => return Ok(err(error)),
                    };

                    (kind, Some(channel))
                }
                _ => return Ok(err("Unsupported event kind")),
            }
        } else if channel_id.is_some() || channel_query.is_some() {
            match resolve_scheduled_event_channel(
                &self.http,
                guild_id,
                channel_id,
                channel_query.as_deref(),
                None,
            )
            .await
            {
                Ok(channel) => match event_kind_for_channel_kind(channel.kind) {
                    Some(kind) => (kind, Some(channel)),
                    None => {
                        if location.is_some() {
                            (ScheduledEventType::External, None)
                        } else {
                            return Ok(err(
                                "channel_query/channel_id must resolve to a voice or stage channel, or provide location for an external event",
                            ));
                        }
                    }
                },
                Err(error) => {
                    if location.is_some() {
                        (ScheduledEventType::External, None)
                    } else {
                        return Ok(err(error));
                    }
                }
            }
        } else if location.is_some() {
            (ScheduledEventType::External, None)
        } else {
            return Ok(err(
                "Provide a voice/stage channel or an external location, or set kind explicitly",
            ));
        };

        let end_time =
            determine_create_end_time(start_time, duration_minutes, explicit_end_time, final_kind);

        let mut builder = CreateScheduledEvent::new(final_kind, name, start_time);
        if let Some(description) = description {
            builder = builder.description(description);
        }
        if let Some(end_time) = end_time {
            builder = builder.end_time(end_time);
        }
        if let Some(channel) = &resolved_channel {
            builder = builder.channel_id(channel.id);
        }
        if final_kind == ScheduledEventType::External
            && let Some(location) = location
        {
            builder = builder.location(location);
        }

        let http = self.http.clone();
        let event = match retry_discord(|| {
            let http = http.clone();
            let builder = builder.clone();
            async move { guild_id.create_scheduled_event(&http, builder).await }
        })
        .await
        {
            Ok(event) => event,
            Err(error) => return Ok(err(format!("Failed to create scheduled event: {error}"))),
        };

        Ok(ok(json!({
            "created": true,
            "event": event_summary_value(&event),
            "resolved_channel": resolved_channel.as_ref().map(channel_summary_value),
        })))
    }
}

impl Tool for ListEvents {
    const NAME: &'static str = "list_events";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Search scheduled events in a guild. Matches against event names, descriptions, ",
                "locations, timestamps, kind, status, and channel id. Returns concise summaries ",
                "instead of raw Discord objects."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild id." },
                    "query": { "type": "string", "description": "Search text for the event name, description, location, timestamps, kind, or status." },
                    "status": { "type": "string", "enum": ["scheduled", "active", "completed", "canceled"], "description": "Optional status filter." },
                    "kind": { "type": "string", "enum": ["voice", "stage", "external"], "description": "Optional event kind filter." },
                    "with_user_count": { "type": "boolean", "description": "Include user counts in results." },
                    "limit": { "type": "integer", "description": "Maximum number of results to return (default 20, max 100)." }
                },
                "required": ["guild_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };

        let query = get_trimmed_string(&args, "query");
        let status_filter = match get_optional_event_status(&args, "status") {
            Ok(value) => value,
            Err(error) => return Ok(err(error)),
        };
        let kind_filter = match get_optional_event_kind(&args, "kind") {
            Ok(value) => value,
            Err(error) => return Ok(err(error)),
        };
        let with_user_count = get_bool(&args, "with_user_count").unwrap_or(false);
        let limit = get_u64(&args, "limit").unwrap_or(SEARCH_LIMIT_DEFAULT as u64);
        let limit = limit.clamp(1, SEARCH_LIMIT_MAX as u64) as usize;

        let http = self.http.clone();
        let events = match fetch_guild_scheduled_events(&http, guild_id, with_user_count).await {
            Ok(events) => events,
            Err(error) => return Ok(err(error)),
        };

        let matches =
            search_scheduled_event_matches(&events, query.as_deref(), status_filter, kind_filter);
        let returned = matches
            .iter()
            .take(limit)
            .map(event_match_to_value)
            .collect::<Vec<_>>();

        Ok(ok(json!({
            "guild_id": guild_id.get(),
            "filters": {
                "query": query,
                "status": status_filter.map(event_status_label),
                "kind": kind_filter.map(event_kind_label),
                "with_user_count": with_user_count,
            },
            "total_matches": matches.len(),
            "returned": returned.len(),
            "events": returned,
        })))
    }
}

impl Tool for UpdateOrCancelEvent {
    const NAME: &'static str = "update_or_cancel_event";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Update an event or cancel it with one tool.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "target_event": { "type": "string" },
                    "action": { "type": "string", "enum": ["update", "cancel"] },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "start_time": { "type": "string" },
                    "duration_minutes": { "type": "integer" },
                    "end_time": { "type": "string" },
                    "kind": { "type": "string" },
                    "channel_id": { "type": "integer" },
                    "channel_query": { "type": "string" },
                    "location": { "type": "string" },
                    "status": { "type": "string" }
                },
                "required": ["guild_id", "target_event", "action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let action = get_string(&args, "action").unwrap_or_else(|| "update".to_string());
        if action == "cancel" {
            // --- Cancel branch (inlined from CancelDiscordScheduledEvent) ---
            let Some(guild_id) = get_guild_id_default(&args) else {
                return Ok(err("guild_id is required"));
            };
            crate::admin_guard_guild!(&self.http, guild_id);

            let Some(target_event) = get_trimmed_string(&args, "target_event") else {
                return Ok(err("target_event is required"));
            };

            let current_event =
                match resolve_scheduled_event_target(&self.http, guild_id, &target_event).await {
                    Ok(event) => event,
                    Err(error) => return Ok(err(error)),
                };

            let http = self.http.clone();
            match retry_discord(|| {
                let http = http.clone();
                async move {
                    guild_id
                        .delete_scheduled_event(&http, current_event.id)
                        .await
                }
            })
            .await
            {
                Ok(()) => Ok(ok(json!({
                    "canceled": true,
                    "deleted": true,
                    "resolved_target": {
                        "input": target_event,
                        "event_id": current_event.id.get(),
                        "name": current_event.name.clone(),
                    },
                    "event": event_summary_value(&current_event),
                }))),
                Err(error) => Ok(err(format!("Failed to delete scheduled event: {error}"))),
            }
        } else {
            // --- Update branch (inlined from UpdateDiscordScheduledEvent) ---
            let Some(guild_id) = get_guild_id_default(&args) else {
                return Ok(err("guild_id is required"));
            };
            crate::admin_guard_guild!(&self.http, guild_id);

            let Some(target_event) = get_trimmed_string(&args, "target_event") else {
                return Ok(err("target_event is required"));
            };

            let current_event =
                match resolve_scheduled_event_target(&self.http, guild_id, &target_event).await {
                    Ok(event) => event,
                    Err(error) => return Ok(err(error)),
                };

            if matches!(
                current_event.status,
                ScheduledEventStatus::Completed | ScheduledEventStatus::Canceled
            ) {
                return Ok(err(
                    "Scheduled events that are completed or canceled cannot be modified",
                ));
            }

            let requested_kind = match get_optional_event_kind(&args, "kind") {
                Ok(value) => value,
                Err(error) => return Ok(err(error)),
            };
            let requested_status = match get_optional_event_status(&args, "status") {
                Ok(value) => value,
                Err(error) => return Ok(err(error)),
            };
            let requested_name = get_trimmed_string(&args, "name");
            let requested_description = get_trimmed_string(&args, "description");
            let requested_start_time = match get_trimmed_string(&args, "start_time") {
                Some(raw) => match parse_smart_timestamp(&raw) {
                    Some(timestamp) => Some(timestamp),
                    None => return Ok(err(format!("Could not parse start_time: {raw}"))),
                },
                None => None,
            };
            let requested_end_time = match get_trimmed_string(&args, "end_time") {
                Some(raw) => match parse_smart_timestamp(&raw) {
                    Some(timestamp) => Some(timestamp),
                    None => return Ok(err(format!("Could not parse end_time: {raw}"))),
                },
                None => None,
            };
            let duration_minutes = match parse_duration_minutes(&args, "duration_minutes") {
                Ok(value) => value,
                Err(error) => return Ok(err(error)),
            };
            let channel_id = get_channel_id(&args, "channel_id");
            let channel_query = get_trimmed_string(&args, "channel_query");
            let location = get_trimmed_string(&args, "location");

            if let Some(end_time) = requested_end_time {
                let comparison_start = requested_start_time.unwrap_or(current_event.start_time);
                if end_time.unix_timestamp() <= comparison_start.unix_timestamp() {
                    return Ok(err("end_time must be after start_time"));
                }
            }

            let mut final_kind = requested_kind.unwrap_or(current_event.kind);
            let mut resolved_channel: Option<GuildChannel> = None;

            if requested_kind.is_none() {
                if location.is_some() {
                    final_kind = ScheduledEventType::External;
                } else if channel_id.is_some() || channel_query.is_some() {
                    let channel = match resolve_scheduled_event_channel(
                        &self.http,
                        guild_id,
                        channel_id,
                        channel_query.as_deref(),
                        None,
                    )
                    .await
                    {
                        Ok(channel) => channel,
                        Err(error) => return Ok(err(error)),
                    };

                    match event_kind_for_channel_kind(channel.kind) {
                        Some(kind) => {
                            final_kind = kind;
                            resolved_channel = Some(channel);
                        }
                        None => {
                            if current_event.kind != ScheduledEventType::External {
                                return Ok(err(
                                    "channel_query/channel_id must resolve to a voice or stage channel, or provide location for an external event",
                                ));
                            }
                        }
                    }
                }
            }

            if let Some(kind) = requested_kind {
                final_kind = kind;
            }

            let kind_changed = final_kind != current_event.kind;

            if matches!(
                final_kind,
                ScheduledEventType::Voice | ScheduledEventType::StageInstance
            ) {
                if channel_id.is_some() || channel_query.is_some() || kind_changed {
                    let desired_channel_kind = channel_kind_for_event_kind(final_kind);
                    let channel = match resolve_scheduled_event_channel(
                        &self.http,
                        guild_id,
                        channel_id,
                        channel_query.as_deref(),
                        desired_channel_kind,
                    )
                    .await
                    {
                        Ok(channel) => channel,
                        Err(error) => {
                            if kind_changed && current_event.kind == ScheduledEventType::External {
                                return Ok(err(format!(
                                    "Changing an external event to {} requires a voice or stage channel",
                                    channel_kind_label(match final_kind {
                                        ScheduledEventType::Voice => ChannelType::Voice,
                                        ScheduledEventType::StageInstance => ChannelType::Stage,
                                        ScheduledEventType::External => ChannelType::Text,
                                        _ => ChannelType::Text,
                                    })
                                )));
                            }
                            return Ok(err(error));
                        }
                    };

                    if !channel_matches_event_kind(&channel, final_kind) {
                        return Ok(err(format!(
                            "Channel {} is not a {} channel",
                            channel.name,
                            channel_kind_label(channel.kind)
                        )));
                    }

                    resolved_channel = Some(channel);
                } else if current_event.kind == ScheduledEventType::External {
                    return Ok(err(
                        "Changing an external event to voice or stage requires channel_id or channel_query",
                    ));
                }
            } else if final_kind == ScheduledEventType::External {
                if kind_changed && location.is_none() {
                    return Ok(err(
                        "location is required when changing a scheduled event to external",
                    ));
                }
                if current_event.kind != ScheduledEventType::External && location.is_none() {
                    return Ok(err("location is required for external scheduled events"));
                }
            }

            let start_time = requested_start_time.unwrap_or(current_event.start_time);
            let end_time = determine_update_end_time(
                start_time,
                &current_event,
                requested_start_time.is_some(),
                duration_minutes,
                requested_end_time,
                final_kind,
            );

            let mut builder = EditScheduledEvent::new();
            let mut changed_fields = Vec::new();

            if let Some(name) = requested_name {
                builder = builder.name(name);
                changed_fields.push("name");
            }
            if let Some(description) = requested_description {
                builder = builder.description(description);
                changed_fields.push("description");
            }
            if let Some(start_time) = requested_start_time {
                builder = builder.start_time(start_time);
                changed_fields.push("start_time");
            }
            if let Some(end_time) = end_time {
                builder = builder.end_time(end_time);
                changed_fields.push("end_time");
            }
            if kind_changed {
                builder = builder.kind(final_kind);
                changed_fields.push("kind");
            }
            if let Some(channel) = &resolved_channel {
                builder = builder.channel_id(channel.id);
                changed_fields.push("channel_id");
            }
            if final_kind == ScheduledEventType::External
                && let Some(location) = location
            {
                builder = builder.location(location);
                changed_fields.push("location");
            }
            if let Some(status) = requested_status {
                let transition_is_valid = match (current_event.status, status) {
                    (ScheduledEventStatus::Scheduled, ScheduledEventStatus::Scheduled)
                    | (ScheduledEventStatus::Scheduled, ScheduledEventStatus::Active)
                    | (ScheduledEventStatus::Scheduled, ScheduledEventStatus::Canceled)
                    | (ScheduledEventStatus::Active, ScheduledEventStatus::Active)
                    | (ScheduledEventStatus::Active, ScheduledEventStatus::Completed) => true,
                    _ if current_event.status == status => true,
                    _ => false,
                };

                if !transition_is_valid {
                    return Ok(err("Invalid status transition for the scheduled event"));
                }

                builder = builder.status(status);
                changed_fields.push("status");
            }

            if changed_fields.is_empty() {
                return Ok(err("No scheduled event fields provided to modify"));
            }

            let http = self.http.clone();
            let event = match retry_discord(|| {
                let http = http.clone();
                let builder = builder.clone();
                async move {
                    guild_id
                        .edit_scheduled_event(&http, current_event.id, builder)
                        .await
                }
            })
            .await
            {
                Ok(event) => event,
                Err(error) => return Ok(err(format!("Failed to modify scheduled event: {error}"))),
            };

            Ok(ok(json!({
                "updated": true,
                "changed_fields": changed_fields,
                "resolved_target": {
                    "input": target_event,
                    "event_id": current_event.id.get(),
                    "name": current_event.name.clone(),
                },
                "resolved_channel": resolved_channel.as_ref().map(channel_summary_value),
                "event": event_summary_value(&event),
            })))
        }
    }
}

impl Tool for GetEventSubscribers {
    const NAME: &'static str = "get_event_subscribers";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get scheduled event subscribers.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "event_id": { "type": "integer" },
                    "limit": { "type": "integer" },
                    "with_member": { "type": "boolean" },
                    "after": { "type": "integer" },
                    "before": { "type": "integer" }
                },
                "required": ["guild_id", "event_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        let Some(event_id) = get_u64(&args, "event_id").map(ScheduledEventId::new) else {
            return Ok(err("event_id is required"));
        };
        let limit = get_u64(&args, "limit");
        let with_member = get_bool(&args, "with_member");

        let pagination = if let Some(after_id) = get_u64(&args, "after") {
            Some(UserPagination::After(serenity::all::UserId::new(after_id)))
        } else {
            get_u64(&args, "before")
                .map(|before_id| UserPagination::Before(serenity::all::UserId::new(before_id)))
        };

        match retry_discord(|| {
            let http = self.http.clone();
            let pagination = pagination.as_ref().map(|value| match value {
                UserPagination::After(id) => UserPagination::After(*id),
                UserPagination::Before(id) => UserPagination::Before(*id),
                _ => unreachable!(),
            });
            async move {
                http.get_scheduled_event_users(guild_id, event_id, limit, pagination, with_member)
                    .await
            }
        })
        .await
        {
            Ok(users) => Ok(ok(to_value(&users))),
            Err(error) => Ok(err(format!("Failed to fetch event subscribers: {error}"))),
        }
    }
}

impl_new!(
    CreateScheduledEventTool,
    ListEvents,
    UpdateOrCancelEvent,
    GetEventSubscribers
);
