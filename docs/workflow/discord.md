# `nekoai-discord` クレートのワークフロー

## 役割

`nekoai-discord` は Discord 接続、イベントループ、コマンド受付、`AgentRuntime` への委譲、ツール登録を担当します。推論そのものは持たず、入出力とルーティングに集中します。

## 主な構成

- `client.rs` (543行): Serenity クライアント生成、全ツールの登録（`register_discord_tools` 関数）、MCP サーバー接続、config-gated ツールの条件付き登録
- `handler.rs` (22行): `EventHandler` 実装（ready イベント → スピナー停止 + 緑色表示）
- `command_router.rs` (83行): Poise フレームワーク設定（`on_error`, `pre_command`, `post_command` フック + `setup` で guild 登録）
- `commands/ask.rs` (115行): `/ask` + `w!ask` コマンド
- `commands/clear.rs` (50行): `/clear` + `w!clear` コマンド
- `commands/history.rs` (58行): `/history` コマンド（slash のみ）
- `commands/utils/session_resolver.rs` (36行): Discord コンテキストから `SessionKey` 判定

## クライアント起動ワークフロー（`DiscordClient::new`）

1. 受け取った `discord_token`, `guild_id`, `agent_runtime`, `config`, `mcp_servers` を使用
2. Gateway Intents: `GUILDS | GUILD_MESSAGES | MESSAGE_CONTENT`
3. Poise コマンドフレームワークを構築（`w!` プレフィックス）
4. `Handler` をイベントハンドラとして登録
5. Serenity `Client` を生成
6. `Arc::new(Http::new(&discord_token))` で HTTP クライアントを生成
7. `ToolRegistry` を作成し、`register_discord_tools()` ですべての Discord ツール名を `ToolAccess::Public` で登録
8. config-gated ツール（`web_search`, `web_fetch`, `code_exec`, `read_file`）を条件付き登録
9. `enabled_names()` で有効なツール名を解決し、実際のツールインスタンスを生成して `AgentRuntime::add_tool()` で登録
10. MCP サーバーに接続し、ツール定義を取得して `McpToolWrapper` でラップして登録

## ツール登録詳細

`register_discord_tools()` 関数で 57 の Discord ツール名を登録（すべて `ToolAccess::Public`）。

| カテゴリ | ツール数 | ツール一覧 |
|---|---|---|
| Channels | 5 | ListChannels, CreateChannelTool, UpdateChannel, ArchiveChannel, SetChannelPermissions |
| Emojis | 4 | ListEmojis, AddEmoji, DeleteEmoji, GetReactionStats |
| Guild | 4 | GetGuildInfo, UpdateGuildSettings, GetAuditLog, ManageBans |
| Invites | 3 | CreateInviteTool, ListInvites, RevokeInvite |
| Members | 8 | SearchMembers, ManageMemberRoles, TimeoutMember, InvestigateMember, ModerateMember, GetMemberActivity, UpdateMemberNickname, KickMember |
| Messages | 9 | SendMessageTool, SearchMessages, BulkDeleteMessages, PinMessage, AddReaction, SendWebhookMessage, FetchReadableChatHistory, CreatePoll, SendAnnouncementWithPin |
| Roles | 12 | ListRoles, UpsertRole, AssignRoles, ReorderRoles, ListRoleMembers, AssignRoleByName, RevokeRoleByName, GetMembersWithRole, ClearRoleFromAllMembers, AssignRoleToMultipleMembers, CreateAndAssignRole, DuplicateRole |
| Schedule | 4 | CreateScheduledEventTool, ListEvents, UpdateOrCancelEvent, GetEventSubscribers |
| Threads | 4 | CreateThreadTool, ListThreads, ArchiveOrLockThread, ManageThreadMembers |
| Voice | 4 | GetVoiceStates（cache も保持）, MoveMemberToVoice, SetVoiceMuteDeafen, ManageStageTopic |
| Web (config-gated) | 2 | SearxngSearch, WebFetch |
| Code/File (config-gated, TODO) | 2 | CodeExec, ReadFile（レジストリ登録のみ、インスタンス化は未実装） |
| MCP | 動的 | McpToolWrapper |

## フレームワーク構築ワークフロー（`command_framework`）

1. コマンド一覧 `ask()`, `clear()`, `history()` を登録
2. Prefix コマンド接頭辞を `w!` に設定
3. `on_error`: `Setup` → panic、`Command` → error ログ、`CommandCheckFailed` → warn ログ、未対応 → `poise::builtins::on_error` 委譲
4. `pre_command`: コマンド実行前に tracing ログ（user_id, channel_id, コマンド名）
5. `post_command`: コマンド実行後に tracing ログ
6. `setup` 内で対象 guild へコマンドを登録
7. `Data { agent_runtime }` をコンテキストに注入

## `/ask` ワークフロー（`w!ask` / `/ask`）

1. Bot ユーザーの実行を除外
2. Slash: `ctx.defer()`、Prefix: `channel_id.start_typing()`
3. `session_resolver` で `SessionKind` と `thread_id` を判定
4. `SessionKey { guild_id, channel_id, thread_id, kind }` を生成
5. `agent_runtime.submit(session_key, Some(user_id), prompt)` を呼び出し
6. 返信テキストを整形: `**ユーザー名**:\n\n{prompt}\n\n**Assistant**:\n\n{response.content}`
7. 2000 文字上限で `split_message`（改行優先分割）し、複数メッセージ送信

## `/clear` ワークフロー（`w!clear` / `/clear`）

1. Bot 実行を除外、`ctx.defer()`
2. `SessionKey` を解決 → `agent_runtime.clear_session(&session_key)`
3. 成功時 "The session cleared."、失敗時 "Failed to clear the session."

## `/history` ワークフロー（`/history` slash のみ）

1. Bot 実行を除外、`ctx.defer()`
2. `SessionKey` を解決 → `agent_runtime.get_history(&session_key)`
3. ターン履歴を `**User**: ...\n**Assistant**: ...` 形式で連結して送信

## セッション解決ワークフロー（`session_resolver`）

`ChannelId` から Discord チャンネル種別を取得し判定:
- Thread（Public/Private/News）: `SessionKind::Thread`, `thread_id = Some(channel_id)`
- Guild 通常チャンネル: `SessionKind::GuildChannel`, `thread_id = None`
- DM: `SessionKind::DirectMessage`, `thread_id = None`
- エラー/未知: `guild_id` 有無で `GuildChannel`/`DirectMessage` フォールバック

## 起動時表示フロー（Handler）

1. `ready` イベント受信
2. スピナーを `finish_and_clear()`
3. `"✓ Discord client ready! Logged in as {bot_name}"` を緑色で表示
4. 以降はイベントループでコマンド待機

## エラー時の挙動

- `on_error`: Setup → panic、Command → error ログ、CommandCheckFailed → warn ログ、その他は委譲
- `/ask` 実行失敗時はエラー文字列をそのまま返信
- `/clear`, `/history` は失敗時に固定エラーメッセージを返信

## 連携ポイント

- `nekoai-agent` (`AgentRuntime`): 推論・履歴・セッションクリア、ツール実行
- `nekoai-tools`: Discord API 連携ツール群（57+ ツール）+ ToolRegistry によるアクセス制御 + MCP ツール
- `nekoai-domain`: `SessionKey` / `SessionKind`
- `nekoai-config`: ツール権限設定・MCP サーバー設定
- `serenity` / `poise`: Discord API とコマンド実行基盤
