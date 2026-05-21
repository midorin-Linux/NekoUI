# `nekoai-cli` クレートのワークフロー

## 役割

`nekoai-cli` はアプリケーションのエントリポイントです。CLI コマンド解釈、起動前初期化、進捗表示、チャットプラットフォーム起動までを担当します。

## 主な構成

- `main.rs` (150行): コマンド定義と実行分岐、プログレスバー表示
- `commands/start.rs` (257行): 起動手順の実体（tracing初期化、設定ロード/自動移行/ウィザード/CLIフォールバック、メモリ初期化）
- `chat.rs` (45行): チャットプラットフォーム（Discord）の抽象 enum + MCPサーバー読み込み

## コマンドワークフロー

`neko` コマンドは現在 `start` サブコマンドのみを持ちます。

1. `clap` で引数を解析
2. `start` が選択されたら `StartCommand::new().await`
3. 初期化成功後、`AgentRuntime::new_with_progress(...)` を実行（`RuntimeInitProgress::TOTAL_STEPS` は 6）
4. `ChatClient::initialize(...)` で MCP サーバーを読み込み、プラットフォーム別クライアント生成
5. `chat_client.run().await` でイベントループ開始

失敗時はエラー表示して `exit(1)`、正常終了時は `exit(0)`。

## `start` の詳細ワークフロー

`StartCommand::start` は以下の順で処理します。

1. ASCII バナー "NEKO AI" を表示、1秒スリープ
2. `init_tracing()` を実行（スピナー表示）
3. **設定の自動移行**: `migrate_json_to_toml()` で旧 `.config/config.json` があれば `.config/config.toml` に変換（`.json.bak` にリネーム）
4. **設定読み込み**:
   - 設定ファイルが存在する場合:
     - `.config/config.toml` 優先、なければ `.config/config.json` からロード
   - 設定ファイルが存在しない場合:
     - **CLIフォールバックモード**（`--skip-setup` フラグ または `DISCORD_AGENT_TOKEN` 環境変数）:
       - 環境変数パス: `config_from_env()` を使用
       - CLI引数パス: `--token`, `--api-key`（→ `NEKOAI_API_KEY`）, `--provider`（→ `NEKOAI_PROVIDER`）, `--model`（→ `NEKOAI_MODEL`）, `--base-url`（→ `NEKOAI_BASE_URL`）, `--guild-id`（→ `NEKOAI_GUILD_ID`）, `--web-search` → `nekoai_setup::cli_fallback::make_config()`
     - **対話型セットアップウィザード**（デフォルト）: `run_setup_wizard().await`（5ステップ、dialoguer ベース）
5. `MemoryStore::new(&config)` を生成（スピナー表示）
6. `memory_store.initialize().await` でベクトルコレクションを準備
7. `memory_store.start_cleanup_job()` で中期記憶の定期クリーンアップを開始
8. `(config, tracing_guard, memory_store)` を返却

処理中は `indicatif` のスピナーで状態を表示します。

## CLI 引数一覧

| 引数 | 説明 |
|---|---|
| `--skip-setup` | セットアップウィザードをスキップ |
| `--token` | Discord ボットトークン |
| `--api-key` | AI プロバイダ API キー（環境変数推奨） |
| `--provider` | プロバイダ名（openai/anthropic/ollama） |
| `--model` | モデル名 |
| `--base-url` | プロバイダベース URL |
| `--guild-id` | Discord ギルド ID |
| `--web-search` | Web 検索機能を有効化 |

## `AgentRuntime` 初期化連携

`main.rs` 側では `RuntimeInitProgress` を使って進捗バーを更新します。

- 総ステップ数: `RuntimeInitProgress::TOTAL_STEPS`（6）
- プログレスバー書式: `[{bar:32.cyan/blue}] {pos:>2}/{len:2} {msg}`
- 成功後に「Agent runtime initialized」を表示

## チャットクライアント選択ワークフロー

`ChatClient::initialize` は最初に `mcp_config::load_mcp_servers()` で MCP サーバー設定を読み込み、`config.chat_platform` で分岐します。

- `ChatPlatform::Discord` の場合:
  1. `DiscordClient::new(token, guild_id, runtime, config, mcp_servers)`
  2. `ChatClient::Discord(client)` を返却

`ChatClient::run` は enum を展開し、該当クライアントの `run` を呼び出します。

## エラー時の挙動

- 起動前初期化の失敗は即時終了
- ログ初期化失敗時は明示メッセージを出して終了
- 設定読み込み失敗時はユーザー向けに原因を表示

## 連携ポイント

- `nekoai-config`: 設定読み込み（TOML/JSON 両対応）
- `nekoai-infra`: ロギング初期化
- `nekoai-memory`: 記憶層初期化
- `nekoai-agent`: 推論ランタイム
- `nekoai-discord`: Discord クライアント（MCP サーバー情報を渡す）
- `nekoai-setup`: ウィザード / CLI フォールバック / 設定移行
