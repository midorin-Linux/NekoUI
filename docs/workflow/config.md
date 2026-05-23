# `nekoui-config` クレート ワークフロー

## 役割

`nekoui-config` は、設定スキーマ定義と設定ファイルの読み込みを担当します。アプリ全体で利用する `Config` 構造体を提供し、各クレートはこの型を通じて設定値へアクセスします。

## 主な構成

- `loader.rs`: すべての設定型、読み込みロジック、`SecretKey` の定義
- `mcp_config.rs`: MCP サーバー設定を `.config/mcp.json` に読み書きする処理
- `lib.rs`: モジュール公開

## 設定スキーマワークフロー

`loader.rs` では、次の設定ツリーを定義します。

### `Config`

```rust
pub struct Config {
    pub chat_platform: ChatPlatform,
    pub discord: Discord,
    pub provider: Provider,
    pub memory: Memory,
    pub tools: ToolPermissions,
    pub web_ui: WebUiConfig,
}
```

### 主要なサブ構造体

- **Discord**: `token`、`guild_id`
- **ChatPlatform**: 現状は `Discord` のみ。`#[serde(rename_all = "snake_case")]`
- **Provider**: `conversation_model`、`summarizer_model`、`embedding_model`
- **ConversationModel**: `provider_base_url`、`api_key`、`model_name`、`parameters`
- **SummarizerModel**: 会話モデルとは別の要約用モデル
- **EmbeddingModel**: `provider_base_url`、`api_key`、`model_name`、`dimension`
- **Parameters**: `max_token`、`temperature`、`top_p`
- **VectorDb**: `url`、`api_key`、`mid_term_collection`、`long_term_collection`
- **Memory**: `vector_db`、`short_term_max_entries`、`mid_term_top_k`、`long_term_top_k`、`mid_term_retention_days`、`long_term_extraction_interval`
- **SearxngConfig**: `base_url`、`max_results`
- **CodeExecConfig**: `allowed_languages`、`timeout_seconds`
- **ReadFileConfig**: `allowed`
- **McpServerConfig**: `name`、`transport`、`command`、`args`、`url`
- **ToolPermissions**: `web_search`、`searxng`、`code_exec`、`read_file`、`code_exec_sandbox`、`read_file_dirs`
- **WebUiConfig**: `bind_address`、`auth_token`、`allowed_origins`

## `SecretKey`

安全なキー型で、内部に `Zeroizing<SecretString>` を保持します。

- `Debug`: 末尾 4 文字のみ表示し、それ以外は `*` でマスク
- `Serialize` / `Deserialize`: 通常の文字列として扱う
- `Drop`: `Zeroizing` によりメモリ上の値を消去

## ロードワークフロー: `Config::load`

1. `config::ConfigBuilder` を作成
2. `.config/config.toml` を優先して読み込む
3. なければ `.config/config.json` をフォールバックとして読む
4. どちらもなければエラー
5. `serde` で `Config` にデシリアライズ
6. 成功したら `Config` を返却

環境変数による上書きは現時点では CLI 側で補完します。

## MCP 設定ワークフロー: `mcp_config.rs`

- `load_mcp_servers()`: `.config/mcp.json` を読み込み、`Vec<McpServerConfig>` を返す。ファイルがなければ空のベクタを返す
- `save_mcp_servers(servers)`: `.config/mcp.json` に整形済み JSON で保存し、`.config/` ディレクトリがなければ作成する

## デフォルト値

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

- ファイルが存在しない場合は `anyhow::Error`
- JSON/TOML の形式が不正な場合は `anyhow::Error`
- 型が一致しない場合は `anyhow::Error`
- 呼び出し元の CLI はこのエラーを受けて起動を止める

## 連携ポイント

- `nekoui-cli`: 起動時の設定読み込み
- `nekoui-agent`: 会話・要約・埋め込みモデル設定の参照
- `nekoui-memory`: 埋め込みモデルと Qdrant 設定の参照
- `nekoui-setup`: 設定ファイルの新規作成とマージ
- `nekoui-discord`: MCP サーバー設定の受け渡し
