# `nekoui-discord` クレート ワークフロー

## 役割

`nekoui-discord` は Discord との接続、イベントループ、コマンド受付、`AgentRuntime` への委譲、ツール登録を担当します。推論そのものは持たず、入出力とルーティングに集中します。

## 主な構成

- `client.rs`: Serenity クライアント生成、Discord ツール登録、MCP サーバー接続、設定で有効化されるツールの条件付き登録
- `handler.rs`: `EventHandler` 実装。`ready` イベントでスピナーを止め、起動完了を表示
- `command_router.rs`: Poise の設定、`on_error`、`pre_command`、`post_command`、`setup` をまとめたコマンドルータ
- `commands/ask.rs`: `/ask` と `w!ask`
- `commands/clear.rs`: `/clear` と `w!clear`
- `commands/history.rs`: `/history`。スラッシュコマンドのみ
- `commands/utils/session_resolver.rs`: Discord コンテキストから `SessionKey` を決定

## クライアント起動ワークフロー: `DiscordClient::new`

1. `discord_token`、`guild_id`、`agent_runtime`、`config`、`mcp_servers` を受け取る
2. Gateway Intents に `GUILDS`、`GUILD_MESSAGES`、`MESSAGE_CONTENT` を設定
3. `w!` プレフィックスの Poise コマンドフレームワークを構築
4. `Handler` をイベントハンドラとして登録
5. Serenity の `Client` を生成
6. `Arc::new(Http::new(&discord_token))` で HTTP クライアントを生成
7. `ToolRegistry` を作成し、Discord の公開ツールを登録
8. 設定で有効な Web 検索・コード実行・ファイル読み取り系ツールを条件付き登録
9. `enabled_names()` で有効なツール名を解決し、実体を生成して `AgentRuntime::add_tool()` で登録
10. MCP サーバーへ接続し、取得したツール定義を `McpToolWrapper` で登録

## ツール登録詳細

`register_discord_tools()` では、Discord 連携ツール群を `ToolAccess::Public` として登録します。カテゴリごとの代表例は次のとおりです。

| カテゴリ | 代表ツール |
|---|---|
| Channels | ListChannels, CreateChannelTool, UpdateChannel, ArchiveChannel, SetChannelPermissions |
| Emojis | ListEmojis, AddEmoji, DeleteEmoji, GetReactionStats |
| Guild | GetGuildInfo, UpdateGuildSettings, GetAuditLog, ManageBans |
| Invites | CreateInviteTool, ListInvites, RevokeInvite |
| Members | SearchMembers, ManageMemberRoles, TimeoutMember, InvestigateMember, ModerateMember, GetMemberActivity, UpdateMemberNickname, KickMember |
| Messages | SendMessageTool, SearchMessages, BulkDeleteMessages, PinMessage, AddReaction, SendWebhookMessage, FetchReadableChatHistory, CreatePoll, SendAnnouncementWithPin |
| Roles | ListRoles, UpsertRole, AssignRoles, ReorderRoles, ListRoleMembers, AssignRoleByName, RevokeRoleByName, GetMembersWithRole, ClearRoleFromAllMembers, AssignRoleToMultipleMembers, CreateAndAssignRole, DuplicateRole |
| Schedule | CreateScheduledEventTool, ListEvents, UpdateOrCancelEvent, GetEventSubscribers |
| Threads | CreateThreadTool, ListThreads, ArchiveOrLockThread, ManageThreadMembers |
| Voice | GetVoiceStates, MoveMemberToVoice, SetVoiceMuteDeafen, ManageStageTopic |
| Web（設定依存） | SearxngSearch, WebFetch |
| Code/File（設定依存） | CodeExec, ReadFile |
| MCP | McpToolWrapper |

## フレームワーク構築ワークフロー: `command_framework`

1. `ask()`、`clear()`、`history()` を登録
2. Prefix コマンドの接頭辞を `w!` に設定
3. `on_error` で `Setup`、`Command`、`CommandCheckFailed` を処理し、それ以外は `poise::builtins::on_error` に委譲
4. `pre_command` で実行前の tracing ログを出力
5. `post_command` で実行後の tracing ログを出力
6. `setup` で対象 guild にコマンドを登録
7. `Data { agent_runtime }` をコンテキストに注入

## `/ask` ワークフロー: `w!ask` / `/ask`

1. Bot ユーザー自身の実行を除外
2. Slash コマンドでは `ctx.defer()`、Prefix コマンドでは `channel_id.start_typing()` を実行
3. `session_resolver` で `SessionKind` と `thread_id` を判定
4. `SessionKey { guild_id, channel_id, thread_id, kind }` を生成
5. `agent_runtime.submit(session_key, Some(user_id), prompt)` を呼び出す
6. 返信を `**ユーザー**:\n\n{prompt}\n\n**Assistant**:\n\n{response.content}` 形式で整形
7. 2000 文字上限を考慮して `split_message` で分割し、複数メッセージとして送信

## `/clear` ワークフロー: `w!clear` / `/clear`

1. Bot 実行を除外し、`ctx.defer()` を呼ぶ
2. `SessionKey` を解決して `agent_runtime.clear_session(&session_key)` を実行
3. 成功時は `The session cleared.`、失敗時は `Failed to clear the session.` を返す

## `/history` ワークフロー: `/history` のみ

1. Bot 実行を除外し、`ctx.defer()` を呼ぶ
2. `SessionKey` を解決して `agent_runtime.get_history(&session_key)` を取得
3. ターン履歴を `**User**: ...\n**Assistant**: ...` 形式で連結して返す

## セッション解決ワークフロー: `session_resolver`

`ChannelId` から Discord のチャンネル種別を判定します。

- Thread（Public / Private / News）: `SessionKind::Thread`、`thread_id = Some(channel_id)`
- Guild の通常チャンネル: `SessionKind::GuildChannel`、`thread_id = None`
- DM: `SessionKind::DirectMessage`、`thread_id = None`
- エラーや未知のケース: `guild_id` の有無で `GuildChannel` / `DirectMessage` にフォールバック

## 起動時表示ワークフロー: `handler`

1. `ready` イベントを受信
2. スピナーを `finish_and_clear()` で終了
3. `Discord client ready! Logged in as {bot_name}` を緑色で表示
4. 以降はイベントループでコマンドを処理

## エラー時の挙動

- `on_error`: `Setup` は panic、`Command` は error ログ、`CommandCheckFailed` は warn ログ、その他は既定のハンドラへ委譲
- `/ask` の失敗時は、返ってきたエラー文をそのまま返信
- `/clear` と `/history` は固定のエラーメッセージを返す

## 連携ポイント

- `nekoui-agent`: 推論、履歴取得、セッションクリア、ツール実行
- `nekoui-tools`: Discord API 連携ツール群、設定依存ツール、MCP ツール
- `nekoui-domain`: `SessionKey`、`SessionKind`
- `nekoui-config`: ツール権限設定、MCP サーバー設定
- `serenity` / `poise`: Discord API とコマンド実行基盤
