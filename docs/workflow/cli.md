# `nekoui-cli` クレート ワークフロー

## 役割

`nekoui-cli` はアプリケーションのエントリポイントです。CLI 引数の解釈、起動前初期化、進捗表示、チャットプラットフォームの起動までを担当します。

## 主な構成

- `main.rs`: コマンド定義、実行分岐、進捗バー表示
- `commands/start.rs`: 起動処理の本体。Tracing 初期化、設定の読み込み、セットアップウィザード、CLI フォールバック、メモリ初期化
- `chat.rs`: チャットプラットフォームを抽象化する enum と、MCP サーバー読み込み

## コマンドワークフロー

`neko` コマンドは現在 `start` サブコマンドのみを持ちます。

1. `clap` で引数を解釈
2. `start` が選ばれたら `StartCommand::new().await` を実行
3. 初期化成功後、`AgentRuntime::new_with_progress(...)` を呼び出す
4. `ChatClient::initialize(...)` で MCP サーバー設定を読み込み、プラットフォーム別クライアントを生成
5. `chat_client.run().await` でイベントループを開始

失敗時はエラーを表示して `exit(1)`、正常終了時は `exit(0)` で終了します。

## `start` の詳細ワークフロー

`StartCommand::start` では次の順で処理します。

1. ASCII バナー `NEKO AI` を表示し、短く待機
2. `init_tracing()` を実行してログを初期化
3. **設定の自動移行**: `migrate_json_to_toml()` により、旧 `.config/config.json` を `.config/config.toml` に変換し、元ファイルは `.json.bak` として退避
4. **設定読み込み**:
   - `.config/config.toml` があればそれを優先し、なければ `.config/config.json` を読む
   - 設定ファイルがない場合は、次のいずれかでフォールバック
     - **CLI フォールバック**: `--skip-setup` または `DISCORD_AGENT_TOKEN` の環境変数
     - **環境変数ベース**: `config_from_env()` を使用
     - **コマンドライン引数ベース**: `--token`、`--api-key`、`--provider`、`--model`、`--base-url`、`--guild-id`、`--web-search` を使って `nekoui_setup::cli_fallback::make_config()` を構築
     - **対話型セットアップ**: `run_setup_wizard().await` を呼び出し、dialoguer ベースのウィザードを実行
5. `MemoryStore::new(&config)` を生成
6. `memory_store.initialize().await` でベクトルコレクションを準備
7. `memory_store.start_cleanup_job()` で中期記憶の定期クリーンアップを開始
8. `(config, tracing_guard, memory_store)` を返却

処理中は `indicatif` のスピナーで状態を表示します。

## CLI 引数一覧

| 引数 | 説明 |
|---|---|
| `--skip-setup` | セットアップウィザードを省略 |
| `--token` | Discord ボットトークン |
| `--api-key` | AI プロバイダの API キー。環境変数の利用も推奨 |
| `--provider` | プロバイダ名。例: `openai` / `anthropic` / `ollama` |
| `--model` | 使用するモデル名 |
| `--base-url` | プロバイダのベース URL |
| `--guild-id` | Discord ギルド ID |
| `--web-search` | Web 検索機能を有効化 |

## `AgentRuntime` 初期化との連携

`main.rs` では `RuntimeInitProgress` を使って進捗バーを更新します。

- 総ステップ数: `RuntimeInitProgress::TOTAL_STEPS`
- プログレスバー書式: `[{bar:32.cyan/blue}] {pos:>2}/{len:2} {msg}`
- 成功後に `Agent runtime initialized` を表示

## チャットクライアント選択ワークフロー

`ChatClient::initialize` は、先に `mcp_config::load_mcp_servers()` で MCP サーバー設定を読み込み、`config.chat_platform` に応じてクライアントを作ります。

- `ChatPlatform::Discord` の場合
  1. `DiscordClient::new(token, guild_id, runtime, config, mcp_servers)` を呼ぶ
  2. `ChatClient::Discord(client)` を返す

`ChatClient::run` は enum を展開し、該当するクライアントの `run` を呼び出します。

## エラー時の挙動

- 起動前初期化に失敗したら即終了
- ログ初期化失敗時は明示的なメッセージを表示して終了
- 設定読み込み失敗時はユーザー向けに原因を表示

## 連携ポイント

- `nekoui-config`: 設定読み込み、TOML/JSON 両対応
- `nekoui-infra`: ロギング初期化
- `nekoui-memory`: 記憶層初期化
- `nekoui-agent`: 推論ランタイム
- `nekoui-discord`: Discord クライアントと MCP サーバー情報の受け渡し
- `nekoui-setup`: ウィザード、CLI フォールバック、設定移行
