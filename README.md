# NekoAI

NekoAI は Rust 製の Discord 用 AI エージェントです。Rig SDK をベースに、Discord コマンド、3 層メモリ、OpenAI 互換モデル設定、セットアップウィザードを備えています。

## 概要

- 現在のチャットプラットフォームは Discord のみです。
- スラッシュコマンド `/ask` / `/clear` / `/history` と、プレフィックスコマンド `w!ask` / `w!clear` を提供します。
- 短期・中期・長期の 3 層メモリを持ちます。
- メモリ用の vector DB には Qdrant 実装と in-memory 実装があります。実行時は中期・長期メモリに Qdrant、短期メモリにインメモリを使います。
- 必要に応じて SearXNG ベースの `web_search` / `web_fetch` を使えます。
- 起動時に `.config/INSTRUCTION.md` をシステムプロンプトとして読み込みます。
- ログは `logs/nekoai.log` に日次ローテーションで出力します。

## リポジトリ構成

- `nekoai-rs/`: Rust backend workspace
- `nekoai-gui/`: React Router ベースのフロントエンドスキャフォールド
- `docs/`: architecture / workflow docs
- `.env.example`: `LOG_LEVEL` の例
- `LICENSE`: Apache-2.0

## バックエンド

`nekoai-rs/` は Cargo workspace で、主なクレートは次の通りです。

- `cli`: `neko` コマンドと起動処理
- `config`: JSON config loader と `SecretKey`
- `setup`: 5 ステップのセットアップウィザードと CLI / env fallback
- `discord`: Serenity + Poise による event handling と command routing
- `agent`: runtime, context, session, provider adapter
- `memory`: short / mid / long-term memory と vector DB abstraction
- `tools`: Discord tools と optional web tools
- `infra`: tracing / logging
- `domain`: shared types

## セットアップ

### 必要条件

- Rust stable
- `just`
- Docker (Qdrant を使う場合)
- Node.js (任意、`nekoai-gui/` を触る場合)

### バックエンド

以下のコマンドは `nekoai-rs/` で実行します。

```bash
git clone <repository-url>
cd NekoAI/nekoai-rs
```

### 設定

- 初回起動で `.config/config.json` がなければ、セットアップウィザードが起動します。
- ウィザードは Discord、provider、model selection、tool permissions、advanced settings の 5 ステップです。
- 既存の `config.json` がある場合は、保存時に既存値を優先してマージされます。
- システムプロンプトは `.config/INSTRUCTION.md` に置きます。なければデフォルトプロンプトが使われます。
- 設定例は `.config/config.json.example` を参照してください。

### CLI / env fallback

- `DISCORD_AGENT_TOKEN` があれば setup wizard はスキップされます。
- `--skip-setup` を使う場合は `--token` が必要です。
- 追加オプションは `--api-key`, `--provider`, `--model`, `--base-url`, `--guild-id`, `--web-search` です。
- 環境変数は `DISCORD_AGENT_TOKEN`, `NEKOAI_API_KEY`, `NEKOAI_PROVIDER`, `NEKOAI_MODEL`, `NEKOAI_BASE_URL`, `NEKOAI_GUILD_ID` です。

```bash
DISCORD_AGENT_TOKEN=... NEKOAI_API_KEY=... NEKOAI_PROVIDER=openai NEKOAI_MODEL=gpt-4o just neko start --skip-setup
```

### ログ

- `LOG_LEVEL` は `nekoai-rs/.env` で設定します。
- 例: `LOG_LEVEL=info` や `LOG_LEVEL=nekoai-agent=debug,nekoai-cli=debug`
- 参照用の `.env.example` がルートにあります。

### Qdrant

- デフォルト URL は `http://localhost:6334` です。
- `mid_term` と `long_term` の collection は起動時に作成されます。

```bash
docker run -d --name qdrant -p 6333:6333 -p 6334:6334 -e QDRANT__SERVICE__GRPC_PORT=6334 -v qdrant_data:/qdrant/storage qdrant/qdrant:latest
```

### 起動

```bash
just neko start
```

以下のコマンドでも起動できます。

```bash
cargo run --bin nekoai-cli -- start
```

## 設定の要点

- `chat_platform`: 現在は `discord` のみです。
- `discord.token`, `discord.guild_id`: Discord 接続と guild-scoped slash command registration に使います。
- `provider.conversation_model`, `provider.summarizer_model`, `provider.embedding_model`: 3 種類のモデルを個別に設定します。
- `memory.vector_db`: Qdrant の URL / API key / collection 名を設定します。
- `memory.short_term_max_entries`, `mid_term_top_k`, `long_term_top_k`, `mid_term_retention_days`, `long_term_extraction_interval`: memory の調整値です。
- `tools.web_search`, `tools.searxng`: SearXNG を使う web search / fetch の有効化です。
- `SecretKey`: token と API key はマスク表示されます。
- 主なデフォルト値は `short_term_max_entries=20`, `mid_term_top_k=3`, `long_term_top_k=5`, `mid_term_retention_days=30`, `long_term_extraction_interval=10` です。
- embedding の既定は `text-embedding-3-small` / `1536` です。
- セットアップウィザードには `openai`, `anthropic`, `ollama`, `custom` のプリセットがあります。
- 実行時は OpenAI 互換 API を使うため、カスタム URL には互換エンドポイントを指定してください。

## Discord コマンド

| Command | Scope | Notes |
|---|---|---|
| `/ask <message>` | slash / `w!ask` | エージェントにメッセージを送信します。長い応答は 2000 文字単位で分割されます。 |
| `/clear` | slash / `w!clear` | 現在のセッションをクリアします。短期メモリはバックグラウンドで mid-term に昇格します。 |
| `/history` | slash only | 直近の会話履歴を表示します。長い履歴は Discord の文字数制限を超える可能性があります。 |

## ツール

- Discord ツールには `message`, `channel`, `guild`, `member`, `role`, `thread`, `voice`, `invite`, `emoji`, `schedule` が含まれます。
- `web_search` と `web_fetch` は `tools.web_search = true` のときだけ登録されます。
- `web_search` は SearXNG を使い、`web_fetch` は読み取り可能なテキストを抽出します。

## 処理の流れ

- Discord メッセージまたはコマンドを受信します。
- session key を解決します。
- mid-term / long-term memory を recall します。
- system instruction、session history、memory をまとめてコンテキストを構築します。
- Rig エージェントがツールサーバー付きで推論を実行します。
- short-term memory と session history を更新します。
- `long_term_extraction_interval` ごとに background extraction を走らせます。

## 開発

### バックエンド

`nekoai-rs/` で実行します。

```bash
just fmt
cargo clippy -- -D warnings
cargo build --bin nekoai-cli
```

- `just fmt` は `cargo +nightly fmt --all` を使います。

### Web UI

`nekoai-gui/` で実行します。

```bash
npm install
npm run dev
npm run build
npm run typecheck
```

## 既知の制限

- メモリの永続化は Qdrant に依存します。
- `/history` は Discord の 2000 文字制限を超える可能性があります。
- web search はデフォルトで無効です。

## 詳細ドキュメント

- `docs/architecture.md`
- `docs/workflow/*.md`

## ライセンス

[Apache-2.0](LICENSE)
