# `nekoai-config` クレートのワークフロー

## 役割

`nekoai-config` は設定スキーマ定義と設定ファイルロードを担当します。アプリ全体で利用される `Config` 構造体を提供し、各クレートはこの型を介して設定値を参照します。

## 主な構成

- `loader.rs` (403行): すべての設定型とロード処理、`SecretKey` 型定義
- `mcp_config.rs` (45行): MCP サーバー設定の個別ファイル（`.config/mcp.json`）読み書き
- `lib.rs` (2行): `pub mod loader; pub mod mcp_config;`

## 設定スキーマワークフロー

`loader.rs` で以下の設定ツリーを定義しています。

### Config（最上位構造体）

```rust
pub struct Config {
    pub chat_platform: ChatPlatform,   // デフォルト: Discord
    pub discord: Discord,              // 必須（デフォルトなし）
    pub provider: Provider,            // 必須（デフォルトなし）
    pub memory: Memory,                // デフォルトあり
    pub tools: ToolPermissions,        // デフォルトあり
    pub web_ui: WebUiConfig,           // デフォルトあり
}
```

### 主要サブ構造体

- **Discord**: `token` (SecretKey), `guild_id` (u64)
- **ChatPlatform**: `Discord`（1 バリアントのみ、`#[serde(rename_all = "snake_case")]`）
- **Provider**: `conversation_model`, `summarizer_model`, `embedding_model` の 3 モデル構成
- **ConversationModel**: `provider_base_url`, `api_key` (SecretKey), `model_name`, `parameters`
- **SummarizerModel**: 同上（会話モデルとは別に指定可能）
- **EmbeddingModel**: `provider_base_url`, `api_key` (SecretKey), `model_name`, `dimension`
- **Parameters**: `max_token` (default: 262144), `temperature` (default: 1.0), `top_p` (default: 0.95)
- **VectorDb**: `url` (default: `http://localhost:6334`), `api_key` (Option), `mid_term_collection` (default: `mid_term`), `long_term_collection` (default: `long_term`)
- **Memory**: `vector_db`, `short_term_max_entries` (20), `mid_term_top_k` (3), `long_term_top_k` (5), `mid_term_retention_days` (30), `long_term_extraction_interval` (10)
- **SearxngConfig**: `base_url` (default: `http://localhost:8080`), `max_results` (5)
- **CodeExecConfig**: `allowed_languages` (default: `["python"]`), `timeout_seconds` (30)
- **ReadFileConfig**: `allowed` (Vec<String>, default: empty)
- **McpServerConfig**: `name`, `transport`, `command` (Option), `args` (Option), `url` (Option)
- **ToolPermissions**: `web_search` (false), `searxng` (SearxngConfig), `code_exec` (false), `read_file` (false), `code_exec_sandbox` (CodeExecConfig), `read_file_dirs` (ReadFileConfig)
- **WebUiConfig**: `bind_address` (default: `127.0.0.1:8080`), `auth_token` (Option), `allowed_origins` (Vec, default: empty)

## SecretKey 型

安全なキー型で `Zeroizing<SecretString>` を内部に持つ。
- `Debug`: 末尾 4 文字のみ表示、全 20 文字に `*` パディング
- `Serialize`/`Deserialize`: 通常の文字列として扱う
- `Drop`: `Zeroizing` によりメモリ上のゼロ化を保証

## ロードワークフロー（`Config::load`）

1. `config::ConfigBuilder` を生成
2. `.config/config.toml` を TOML 形式で優先ロード
3. なければ `.config/config.json` を JSON 形式でフォールバック
4. 両方なければエラー
5. `serde` で `Config` にデシリアライズ
6. 成功時は `Config` を返却

環境変数オーバーライドは未実装（CLI 側で別途対応）。

## MCP 設定ワークフロー（`mcp_config.rs`）

- `load_mcp_servers()`: `.config/mcp.json` を読み込み、`Vec<McpServerConfig>` を返却（ファイル不在時は空 vec）
- `save_mcp_servers(servers)`: `.config/mcp.json` に prettified JSON で保存（`.config/` ディレクトリがなければ作成）

## デフォルト値

主要なデフォルト:
- `chat_platform`: `Discord`
- `memory.vector_db.url`: `http://localhost:6334`
- `memory.vector_db.mid_term_collection`: `mid_term`
- `memory.vector_db.long_term_collection`: `long_term`
- `memory.short_term_max_entries`: `20`
- `memory.mid_term_top_k`: `3`
- `memory.long_term_top_k`: `5`
- `memory.mid_term_retention_days`: `30`
- `memory.long_term_extraction_interval`: `10`
- `tools.searxng.base_url`: `http://localhost:8080`
- `tools.searxng.max_results`: `5`
- `tools.code_exec_sandbox.timeout_seconds`: `30`
- `web_ui.bind_address`: `127.0.0.1:8080`

## エラー時の挙動

- ファイル不在: `anyhow::Error` を返却
- JSON/TOML 形式不正: `anyhow::Error` を返却
- 型不一致: `anyhow::Error` を返却
- 呼び出し側（CLI）がこれを受けて起動を停止

## 連携ポイント

- `nekoai-cli`: 起動時ロード
- `nekoai-agent`: 会話/要約モデル・API・パラメータ参照
- `nekoai-memory`: 埋め込みモデル・Qdrant 設定参照
- `nekoai-setup`: 設定ファイルの新規作成・マージ
- `nekoai-discord`: MCP サーバー設定の受け渡し
