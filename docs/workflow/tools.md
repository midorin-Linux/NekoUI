# `nekoai-tools` クレートのワークフロー

## 役割

`nekoai-tools` は、Rig SDK の `Tool` trait を実装したエージェント用ツールの集まりです。Discord API 連携ツール、Web 検索ツール、コード実行ツール、ファイル読み取りツール、MCP ツールを提供します。

## 主な構成（22ファイル、合計 7,708行）

```
nekoai-rs/tools/src/
├── lib.rs                  (6行)  # pub mod code_exec, discord, mcp, read_file, registry, search
├── registry.rs            (103行) # ToolRegistry + ToolAccess (Public/ConfigGated/Mcp) + ConfigGate
├── code_exec.rs           (248行) # CodeExec（サンドボックスコード実行）
├── read_file.rs           (216行) # ReadFile（許可ディレクトリからのファイル読み取り）
├── search.rs              (604行) # SearxngSearch（Web検索）+ WebFetch（URL取得、SSRF対策）
├── mcp/
│   ├── mod.rs              (1行)
│   └── client.rs          (185行) # McpClient（stdio/sse接続）+ McpToolWrapper（Rig Tool ラッパー）
└── discord/
    ├── mod.rs              (45行) # モジュール宣言 + pub use 再エクスポート
    ├── error.rs            (39行) # DiscordToolError（Serenity/Json/Tool バリアント）
    ├── helpers.rs         (386行) # 出力構築(ok/err)、引数パース、Enum変換、リトライ、ID解決、impl_new! マクロ
    ├── permission.rs      (106行) # 管理者権限ガード関数 + admin_guard_*! マクロ
    ├── channel.rs         (407行) # 5ツール（ListChannels, CreateChannel, Update, Archive, SetPermissions）
    ├── message.rs         (808行) # 9ツール（Send, Search, BulkDelete, Pin, Reaction, Webhook, FetchHistory, Poll, Announce）
    ├── guild.rs           (289行) # 4ツール（GetGuildInfo, UpdateSettings, AuditLog, ManageBans）
    ├── role.rs           (1251行) # 12ツール（List, Upsert, Assign, Reorder, ListMembers, ByName, Duplicate 等）
    ├── member.rs          (818行) # 8ツール（Search, ManageRoles, Timeout, Investigate, Moderate, Kick, Activity, Nickname）
    ├── thread.rs          (294行) # 4ツール（Create, List, Archive/Lock, ManageMembers）
    ├── voice.rs           (369行) # 4ツール（GetVoiceStates, Move, Mute/Deafen, StageTopic）
    ├── invite.rs          (165行) # 3ツール（List, Create, Revoke）
    ├── emoji.rs           (232行) # 4ツール（List, Add, Delete, ReactionStats）
    └── schedule.rs       (1516行) # 4ツール（Create, List, Update/Cancel, Subscribers）
```

## ツール実装パターン

全 54 のツール構造体が `rig::tool::Tool` trait を実装します。

### パターンA: 型付き引数（`SendMessageTool`, `CreatePoll`, `BulkDeleteMessages`）

```rust
#[derive(Deserialize)]
pub struct SendMessageArgs { pub channel_id: u64, pub message: String }

impl Tool for SendMessageTool {
    const NAME: &'static str = "send_message";
    type Error = DiscordToolError;
    type Args = SendMessageArgs;
    type Output = Value;
    // ...
}
```

### パターンB: Value ベース（上記以外の全ツール）

```rust
impl Tool for EditDiscordMessage {
    const NAME: &'static str = "edit_discord_message";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;
    // call() 内で helpers::get_* 関数でパース
}
```

### `GetVoiceStates` のみ特殊

`http` に加えて `cache: Arc<Cache>` を保持（`impl_new!` マクロ不使用）。

## 共通ヘルパー（`helpers.rs`）

### 出力構築

- `ok(data)`: `{ ok: true, data: ... }`
- `err(message)`: `{ ok: false, error: ... }`
- `to_value(value)`: `Serialize` → `Value`

### 引数パース

`get_u64/u32/u16/u8/bool/string/u64_list`, `get_channel_id/user_id/message_id`, `get_guild_id_default`

### Enum 変換

`parse_channel_type`, `parse_thread_type`, `parse_auto_archive_duration`, `parse_scheduled_event_type/status`, `parse_timestamp`, `parse_colour`, `parse_reaction_type`, `parse_relative_time`

### リトライ

`retry_discord(f)`: 指数バックオフ（100ms~10s）+ jitter、5回リトライ

### ID解決

`resolve_user_id`, `resolve_role_id`, `resolve_role_ids`: "name", "@mention", "123456789" のいずれかを受け付け

### その他

- `resolve_relative_timestamp("10m", "1h", "1d")`: 相対時間→Timestamp
- `snowflake_to_datetime`: Snowflake→DateTime
- `fetch_guild_members`: ページネーション対応メンバー一覧取得
- `impl_new!`: `pub fn new(http: Arc<Http>) -> Self` を生成するマクロ

## 権限モジュール（`permission.rs`）

### 関数

- `require_admin(http, guild_id, user_id)`: 特定ユーザーの管理者確認
- `require_current_user_admin(http, guild_id)`: Bot 自身の管理者確認
- `require_current_user_admin_for_channel(http, channel_id)`: チャンネル経由
- `require_current_user_admin_for_invite_code(http, code)`: 招待コード経由

### マクロ

- `admin_guard_guild!($http, $guild_id)`: 管理者チェック + `return Ok(err(...))`
- `admin_guard_channel!($http, $channel_id)`: 同上
- `admin_guard_invite!($http, $code)`: 同上

権限不足時は `return Ok(err("..."))` で即座にエラー応答。

## ToolRegistry（`registry.rs`）

ツールのアクセス制御と有効/無効管理:

```rust
pub enum ToolAccess { Public, ConfigGated(ConfigGate), Mcp }
pub enum ConfigGate { WebSearch, CodeExec, ReadFile }
```

- `register(name, access)`: ツール名とアクセスレベルを登録
- `is_enabled(name, permissions)`: アクセスレベル + パーミッション設定から有効/無効判定
- `enabled_names(permissions)`: 有効なツール名一覧を返却
- `public_names()`: 全 Public ツール名一覧
- `all_names()`: 全ツール名一覧

## ツール一覧（全 54 構造体）

### Low-level tools（`discord_` 接頭辞、全 41）

| モジュール | ツール数 | 内訳 |
|---|---|---|
| message | 9 | Send, Edit, Delete, Get, BulkDelete, History, Pin, Unpin, AddReaction, RemoveReaction → 高レベル版に統合 |
| channel | 5 | Create, Delete, Modify, GetInfo, List |
| guild | 4 | GetInfo, List, Modify, AuditLog |
| role | 6 | List, Create, Delete, Modify, AddToMember, RemoveFromMember |
| member | 8 | List, Info, Kick, Ban, Unban, BulkBan, Modify, Timeout |
| thread | 4 | Create, Delete, List, AddMember |
| voice | 4 | Move, Disconnect, Mute, Deafen |
| invite | 3 | List, Create, Delete |
| emoji | 4 | List, Create, Delete, Stickers |
| schedule | 4 | Search, Schedule, Update, Cancel |

### High-level tools（エージェント向けラッパー、全 48）

| モジュール | ツール名 | 説明 |
|---|---|---|
| **message** | `send_message` | 送信（admin_guard 付き） |
| | `search_messages` | キーワード検索 |
| | `bulk_delete_messages` | 一括削除（admin_guard 付き） |
| | `pin_message` | ピン・アンピン・一覧（統合） |
| | `add_reaction` | リアクション追加 |
| | `send_webhook_message` | Webhook 送信 |
| | `fetch_readable_chat_history` | LLM 向け整形済み履歴 |
| | `search_channel_messages` | チャンネル内メッセージ検索 |
| | `create_poll` | 投票作成（自動リアクション付き） |
| | `send_announcement_with_pin` | お知らせ送信＋自動ピン |
| **channel** | `list_channels` | 一覧（情報付き） |
| | `create_channel` | 作成（admin_guard 付き） |
| | `update_channel` | 更新（admin_guard 付き） |
| | `archive_channel` | アーカイブ（読み取り専用化） |
| | `set_channel_permissions` | 権限設定 |
| **guild** | `get_guild_info` | 情報取得 |
| | `update_guild_settings` | 設定更新（admin_guard 付き） |
| | `get_audit_log` | 監査ログ |
| | `manage_bans` | BAN 管理 |
| **role** | `list_roles` | 一覧 |
| | `upsert_role` | 作成/更新（role_id 有無で自動切替） |
| | `assign_roles` | 複数メンバーに付与/剥奪 |
| | `reorder_roles` | 並び替え |
| | `list_role_members` | 保持メンバー一覧 |
| | `assign_role_by_name` | 名前解決して付与 |
| | `revoke_role_by_name` | 名前解決して剥奪 |
| | `get_members_with_role` | 保持メンバー検索 |
| | `clear_role_from_all_members` | 全メンバーから一括削除 |
| | `assign_role_to_multiple_members` | 複数ユーザーに一括付与 |
| | `create_and_assign_role` | 作成＋即時付与 |
| | `duplicate_role` | 既存ロールの複製 |
| **member** | `search_members` | 名前/ロール/Timeout 状態で検索 |
| | `manage_member_roles` | 名前解決してロール操作 |
| | `timeout_member` | 相対時間で Timeout |
| | `investigate_member` | 詳細プロファイル |
| | `moderate_member` | Kick/Ban/Softban 統合 |
| | `get_member_activity` | アクティビティ情報 |
| | `update_member_nickname` | ニックネーム変更 |
| | `kick_member` | Kick（admin_guard 付き） |
| **thread** | `create_thread` | 作成（admin_guard 付き） |
| | `list_threads` | アクティブ一覧 |
| | `archive_or_lock_thread` | アーカイブ/ロック |
| | `manage_thread_members` | 追加/削除/一覧 |
| **voice** | `get_voice_states` | ボイスチャンネル一覧 |
| | `move_member_to_voice` | 移動 |
| | `set_voice_mute_deafen` | ミュート/デフ一括設定 |
| | `manage_stage_topic` | ステージトピック管理 |
| **invite** | `create_invite` | 作成（制限付き） |
| | `list_invites` | 一覧 |
| | `revoke_invite` | 削除 |
| **emoji** | `list_emojis` | 一覧 |
| | `add_emoji` | 追加 |
| | `delete_emoji` | 削除 |
| | `get_reaction_stats` | リアクション統計 |
| **schedule** | `list_events` | 一覧 |
| | `create_scheduled_event_tool` | 作成 |
| | `update_or_cancel_event` | 更新/キャンセル |
| | `get_event_subscribers` | 参加者一覧 |

### 非 Discord ツール（全 5）

| ツール名 | 説明 | アクセスレベル |
|---|---|---|
| `web_search` | SearXNG 経由 Web 検索 | ConfigGated(WebSearch) |
| `web_fetch` | URL 取得 + HTML パース（SSRF 対策） | ConfigGated(WebSearch) |
| `code_exec` | サンドボックスコード実行（Python/Rust/JS） | ConfigGated(CodeExec) |
| `read_file` | 許可ディレクトリからのファイル読み取り | ConfigGated(ReadFile) |
| `mcp_*` | MCP サーバーツール（動的名称） | Mcp |

## MCP クライアント（`mcp/client.rs`）

- `McpClient::connect(config)`: stdio（子プロセス）/ SSE（HTTP ストリーミング）で MCP サーバーに接続
- `tool_defs()`: サーバーからツール定義一覧を取得（キャッシュ）
- `call_tool(name, args)`: ツールを呼び出し
- `McpToolWrapper`: MCP ツールを `rig::tool::Tool` としてラップ（名前は `mcp_{server}_{tool}`）

## ツール新規追加の手順

1. 対応するモジュールファイルに新しいツール構造体を追加
2. `Tool` trait を実装（`definition` + `call`）
3. 管理者権限が必要なら `admin_guard_*!` マクロを `call()` 内で呼ぶ
4. `mod.rs` の `pub use` と `pub mod` に追加
5. `discord::client.rs` の `register_discord_tools()` 関数 + インスタンス化ブロックに追加
6. （必要に応じて）`helpers.rs` にパーサー関数を追加

## 連携ポイント

- `nekoai-agent`: `ToolServerHandle` を介したツール実行（`InstrumentedTool` ラッパー）
- `nekoai-discord`: 起動時に全ツールを `AgentRuntime::add_tool()` で登録
- `nekoai-config`: `ToolPermissions` による有効/無効制御
- `serenity`: Discord API 呼び出し基盤
