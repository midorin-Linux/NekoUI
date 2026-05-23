# `nekoui-tools` クレート ワークフロー

## 役割

`nekoui-tools` は、Rig SDK の `Tool` trait を実装したエージェント向けツール群を提供します。Discord API 連携、Web 検索、コード実行、ファイル読み取り、MCP ツールを含みます。

## 主な構成

```text
nekoui-rs/tools/src/
├── lib.rs               # code_exec, discord, mcp, read_file, registry, search を公開
├── registry.rs          # ToolRegistry と ToolAccess / ConfigGate
├── code_exec.rs         # CodeExec サンドボックスでのコード実行
├── read_file.rs         # 許可されたディレクトリからのファイル読み取り
├── search.rs            # SearxngSearch、WebFetch、URL 取得、SSRF 対策
├── mcp/
│   ├── mod.rs
│   └── client.rs        # McpClient と McpToolWrapper
└── discord/
    ├── mod.rs           # モジュール宣言と再エクスポート
    ├── error.rs         # DiscordToolError
    ├── helpers.rs       # 出力整形、引数パース、Enum 変換、リトライ、ID 解決
    ├── permission.rs    # 権限ガード関数と admin_guard_* マクロ
    ├── channel.rs
    ├── message.rs
    ├── guild.rs
    ├── role.rs
    ├── member.rs
    ├── thread.rs
    ├── voice.rs
    ├── invite.rs
    ├── emoji.rs
    └── schedule.rs
```

## ツール実装パターン

全体で多数のツール構造体が `rig::tool::Tool` trait を実装します。

### パターン A: 型付き引数

`SendMessageTool`、`CreatePoll`、`BulkDeleteMessages` などで使います。

```rust
#[derive(Deserialize)]
pub struct SendMessageArgs {
    pub channel_id: u64,
    pub message: String,
}

impl Tool for SendMessageTool {
    const NAME: &'static str = "send_message";
    type Error = DiscordToolError;
    type Args = SendMessageArgs;
    type Output = Value;
}
```

### パターン B: `Value` ベース

それ以外の多くのツールで使います。`call()` 内で `helpers::get_*` 関数を使ってパースします。

```rust
impl Tool for EditDiscordMessage {
    const NAME: &'static str = "edit_discord_message";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;
}
```

### `GetVoiceStates` の特殊ケース

`http` に加えて `cache: Arc<Cache>` を保持します。`impl_new!` マクロは使いません。

## 共通ヘルパー: `helpers.rs`

### 出力整形

- `ok(data)`: `{ ok: true, data: ... }`
- `err(message)`: `{ ok: false, error: ... }`
- `to_value(value)`: `Serialize` を `Value` に変換

### 引数パース

`get_u64` / `get_u32` / `get_u16` / `get_u8` / `get_bool` / `get_string` / `get_u64_list`、`get_channel_id` / `get_user_id` / `get_message_id`、`get_guild_id_default`

### Enum 変換

`parse_channel_type`、`parse_thread_type`、`parse_auto_archive_duration`、`parse_scheduled_event_type`、`parse_scheduled_event_status`、`parse_timestamp`、`parse_colour`、`parse_reaction_type`、`parse_relative_time`

### リトライ

- `retry_discord(f)`: 指数バックオフ、jitter、複数回リトライ

### ID 解決

- `resolve_user_id`、`resolve_role_id`、`resolve_role_ids`: `name`、`@mention`、`123456789` のいずれかを受け付ける

### その他

- `resolve_relative_timestamp("10m", "1h", "1d")`: 相対時間を Timestamp に変換
- `snowflake_to_datetime`: Snowflake を `DateTime` に変換
- `fetch_guild_members`: ページネーション対応のメンバー取得
- `impl_new!`: `pub fn new(http: Arc<Http>) -> Self` を生成するマクロ

## 権限モジュール: `permission.rs`

### 関数

- `require_admin(http, guild_id, user_id)`: 特定ユーザーの管理権限を確認
- `require_current_user_admin(http, guild_id)`: Bot 自身の管理権限を確認
- `require_current_user_admin_for_channel(http, channel_id)`: チャンネル経由で確認
- `require_current_user_admin_for_invite_code(http, code)`: 招待コード経由で確認

### マクロ

- `admin_guard_guild!($http, $guild_id)`: 権限チェックを行い、失敗時は `return Ok(err(...))`
- `admin_guard_channel!($http, $channel_id)`: 同様
- `admin_guard_invite!($http, $code)`: 同様

権限不足時は即座にエラー応答を返します。

## `ToolRegistry`: `registry.rs`

ツールのアクセス制御と有効/無効管理を行います。

```rust
pub enum ToolAccess { Public, ConfigGated(ConfigGate), Mcp }
pub enum ConfigGate { WebSearch, CodeExec, ReadFile }
```

- `register(name, access)`: ツール名とアクセスレベルを登録
- `is_enabled(name, permissions)`: アクセスレベルと設定から有効か判定
- `enabled_names(permissions)`: 有効なツール名一覧を返す
- `public_names()`: Public ツール名一覧を返す
- `all_names()`: 全ツール名一覧を返す

## ツール一覧

### Low-level tools: `discord_` 接頭辞、約 41 個

| モジュール | 内容 |
|---|---|
| message | 送信、編集、削除、取得、一括削除、履歴、ピン、リアクションなど |
| channel | 作成、削除、更新、情報取得、一覧 |
| guild | 情報取得、一覧、更新、監査ログ |
| role | 一覧、作成、削除、更新、メンバーへの付与/解除 |
| member | 一覧、情報、Kick、Ban、Unban、BulkBan、更新、Timeout |
| thread | 作成、削除、一覧、メンバー追加 |
| voice | 移動、切断、ミュート、デフェン、ステージ操作 |
| invite | 一覧、作成、削除 |
| emoji | 一覧、作成、削除、スタンプ関連 |
| schedule | 検索、作成、更新、キャンセル |

### High-level tools: エージェント向けラッパー、約 48 個

| モジュール | ツール名 | 説明 |
|---|---|---|
| message | `send_message` | 送信。`admin_guard` 付き |
| message | `search_messages` | キーワード検索 |
| message | `bulk_delete_messages` | 一括削除。`admin_guard` 付き |
| message | `pin_message` | ピン/アンピン/一覧を統合 |
| message | `add_reaction` | リアクション追加 |
| message | `send_webhook_message` | Webhook 送信 |
| message | `fetch_readable_chat_history` | LLM 向け整形済み履歴 |
| message | `search_channel_messages` | チャンネル内メッセージ検索 |
| message | `create_poll` | 投票作成。自動リアクション付き |
| message | `send_announcement_with_pin` | お知らせ送信と自動ピン |
| channel | `list_channels` | 一覧。情報付き |
| channel | `create_channel` | 作成。`admin_guard` 付き |
| channel | `update_channel` | 更新。`admin_guard` 付き |
| channel | `archive_channel` | アーカイブして読み取り専用化 |
| channel | `set_channel_permissions` | 権限設定 |
| guild | `get_guild_info` | 情報取得 |
| guild | `update_guild_settings` | 設定更新。`admin_guard` 付き |
| guild | `get_audit_log` | 監査ログ |
| guild | `manage_bans` | BAN 管理 |
| role | `list_roles` | 一覧 |
| role | `upsert_role` | 作成/更新。`role_id` の有無で切り替え |
| role | `assign_roles` | 複数メンバーに付与/剥奪 |
| role | `reorder_roles` | 並び替え |
| role | `list_role_members` | 保持メンバー一覧 |
| role | `assign_role_by_name` | 名前解決して付与 |
| role | `revoke_role_by_name` | 名前解決して剥奪 |
| role | `get_members_with_role` | 保持メンバー検索 |
| role | `clear_role_from_all_members` | 全メンバーから一括削除 |
| role | `assign_role_to_multiple_members` | 複数ユーザーに一括付与 |
| role | `create_and_assign_role` | 作成後に即時付与 |
| role | `duplicate_role` | 既存ロールの複製 |
| member | `search_members` | 名前、ロール、Timeout 状態で検索 |
| member | `manage_member_roles` | 名前解決してロール操作 |
| member | `timeout_member` | 相対時間で Timeout |
| member | `investigate_member` | 詳細プロファイル取得 |
| member | `moderate_member` | Kick / Ban / Softban を統合 |
| member | `get_member_activity` | アクティビティ確認 |
| member | `update_member_nickname` | ニックネーム変更 |
| member | `kick_member` | Kick。`admin_guard` 付き |
| thread | `create_thread` | 作成。`admin_guard` 付き |
| thread | `list_threads` | アクティブ一覧 |
| thread | `archive_or_lock_thread` | アーカイブ / ロック |
| thread | `manage_thread_members` | 追加 / 削除 / 一覧 |
| voice | `get_voice_states` | ボイスチャンネル一覧 |
| voice | `move_member_to_voice` | 移動 |
| voice | `set_voice_mute_deafen` | ミュート / デフェンを一括設定 |
| voice | `manage_stage_topic` | ステージトピック管理 |
| invite | `create_invite` | 制限付き作成 |
| invite | `list_invites` | 一覧 |
| invite | `revoke_invite` | 削除 |
| emoji | `list_emojis` | 一覧 |
| emoji | `add_emoji` | 追加 |
| emoji | `delete_emoji` | 削除 |
| emoji | `get_reaction_stats` | リアクション統計 |
| schedule | `list_events` | 一覧 |
| schedule | `create_scheduled_event_tool` | 作成 |
| schedule | `update_or_cancel_event` | 更新 / キャンセル |
| schedule | `get_event_subscribers` | 参加者一覧 |

### 設定依存の Discord ツール: 5 個

| ツール名 | 説明 | アクセスレベル |
|---|---|---|
| `web_search` | SearXNG 経由の Web 検索 | `ConfigGated(WebSearch)` |
| `web_fetch` | URL 取得と HTML パース。SSRF 対策あり | `ConfigGated(WebSearch)` |
| `code_exec` | サンドボックスでのコード実行。Python / Rust / JS | `ConfigGated(CodeExec)` |
| `read_file` | 許可ディレクトリ内のファイル読み取り | `ConfigGated(ReadFile)` |
| `mcp_*` | MCP サーバー由来の動的ツール名 | `Mcp` |

## MCP クライアント: `mcp/client.rs`

- `McpClient::connect(config)`: stdio、子プロセス、SSE、HTTP ストリーミングで MCP サーバーに接続
- `tool_defs()`: サーバーからツール定義一覧を取得し、キャッシュする
- `call_tool(name, args)`: ツールを呼び出す
- `McpToolWrapper`: MCP ツールを `rig::tool::Tool` としてラップし、`mcp_{server}_{tool}` 形式で公開

## 新しいツールを追加する手順

1. 対応するモジュールに新しいツール構造体を追加
2. `Tool` trait を実装し、`definition` と `call` を用意
3. 権限が必要なら `admin_guard_*` マクロを `call()` 内で呼ぶ
4. `mod.rs` の `pub use` と `pub mod` に追加
5. `discord::client.rs` の `register_discord_tools()` と生成ブロックに追加
6. 必要に応じて `helpers.rs` にパーサー関数を追加

## 連携ポイント

- `nekoui-agent`: `ToolServerHandle` を介したツール実行、`InstrumentedTool` ラップ
- `nekoui-discord`: 起動時に全ツールを `AgentRuntime::add_tool()` で登録
- `nekoui-config`: `ToolPermissions` による有効/無効制御
- `serenity`: Discord API 呼び出し基盤
