# Discord AI Agent — アーキテクチャ設計書

> Rust + Serenity + Rig を基盤とした、拡張可能な Discord 用 AI エージェント

---

## 目次

1. [全体像](#1-全体像)
2. [リポジトリ構造](#2-リポジトリ構造)
3. [クレート責務一覧](#3-クレート責務一覧)
4. [起動シーケンス](#4-起動シーケンス)
5. [Entry / CLI 層](#5-entry--cli-層)
6. [Setup / 初回セットアップ層](#6-setup--初回セットアップ層)
7. [Discord Gateway 層](#7-discord-gateway-層-serenity)
8. [Agent Core 層](#8-agent-core-層-rig)
9. [Memory 層（3層設計）](#9-memory-層3層設計)
10. [Tool System 層](#10-tool-system-層)
11. [Infrastructure 層](#11-infrastructure-層)
12. [設定スキーマ](#12-設定スキーマ)
13. [データフロー（エンドツーエンド）](#13-データフローエンドツーエンド)
14. [セキュリティ・権限モデル](#14-セキュリティ権限モデル)
15. [Web UI 拡張戦略](#15-web-ui-拡張戦略)
16. [ビルド・開発運用](#16-ビルドと開発運用)
17. [設計上のトレードオフ](#17-設計上のトレードオフ)

---

## 1. 全体像

Discord AI Agent は **Cargo workspace 形式のモノレポ**で構成され、以下の 3 原則に基づいて設計されています。

- **層ごとの単一責務**: 各クレートは明確な役割を持ち、上位層は下位層の実装詳細を知らない
- **初回体験の優先**: 設定ファイル不在を自動検知し、TUI または CLI で完結するオンボーディングを提供する
- **拡張を前提とした設計**: Web UI・MCP サーバ・追加ツールを、コア層を変更せずに追加できる

```
外部
  │
  ├─ Discord WebSocket ──▶ Discord Gateway 層 (Serenity)
  │                              │
  │                              ▼
  │                       Agent Core 層 (Rig)
  │                         │        │
  │                         ▼        ▼
  │                    Tool System  ProviderAdapter (Claude / OpenAI)
  │                         │
  │                         ▼
  │                    Memory 層（3層）
  │                   ┌──────────────────────────────────────┐
  │                   │ 短期: セッションメモリ (DashMap)      │
  │                   │ 中期: 会話サマリー  (Vector DB)       │
  │                   │ 長期: 重要情報      (Vector DB)       │
  │                   └──────────────────────────────────────┘
  │                         │
  │                         ▼
  └─ (将来) Web UI ◀── Infrastructure 層 (SQLite / Vector DB / EventBus / Axum)
```

---

## 2. リポジトリ構造

```
discord-agent/
├── Cargo.toml                  # workspace 定義
├── Cargo.lock
├── justfile                    # タスクランナー (fmt / test / run / schema)
├── .env.example
│
├── crates/
│   ├── cli/                    # エントリポイント・サブコマンド
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── commands/
│   │           ├── run.rs
│   │           ├── config.rs
│   │           └── mcp_server.rs
│   │
│   ├── setup/                  # 初回 TUI ウィザード
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── wizard.rs       # ratatui ベースの UI
│   │       ├── steps/
│   │       │   ├── token.rs
│   │       │   ├── provider.rs
│   │       │   └── tools.rs
│   │       └── cli_fallback.rs # --token 等フラグによる非 TUI セットアップ
│   │
│   ├── config/                 # 設定スキーマ・検証・ロード
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs       # Config 型定義 (serde)
│   │       ├── loader.rs       # ファイル探索・環境変数オーバーライド
│   │       └── validator.rs
│   │
│   ├── discord/                # Serenity ラッパ・イベント処理
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── handler.rs      # EventHandler 実装
│   │       ├── command_router.rs
│   │       ├── context_resolver.rs
│   │       └── typing_indicator.rs
│   │
│   ├── agent/                  # Rig エージェントループ・セッション管理
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime.rs      # AgentRuntime (推論ループ)
│   │       ├── session.rs      # SessionManager
│   │       ├── context.rs      # ContextManager (圧縮・サマリー・記憶統合)
│   │       └── provider.rs     # ProviderAdapter
│   │
│   ├── memory/                 # 3層記憶システム
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── store.rs        # MemoryStore (3層の統合インターフェース)
│   │       ├── short_term.rs   # 短期記憶: セッション内インメモリ
│   │       ├── mid_term.rs     # 中期記憶: 会話サマリーのベクトル保存
│   │       ├── long_term.rs    # 長期記憶: 重要情報のベクトル保存
│   │       ├── embedding.rs    # テキスト→ベクトル変換
│   │       └── vector_db/
│   │           ├── mod.rs      # VectorDbClient トレイト
│   │           ├── qdrant.rs   # Qdrant 実装
│   │           └── in_memory.rs # テスト用インメモリ実装
│   │
│   ├── tools/                  # ツールレジストリ・実行・権限
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs     # ToolRegistry
│   │       ├── permission.rs   # Permission guard
│   │       ├── abort.rs        # AbortHandle
│   │       ├── builtin/
│   │       │   ├── web_search.rs
│   │       │   └── code_exec.rs
│   │       └── mcp/
│   │           ├── client.rs   # MCP クライアント
│   │           └── transport/
│   │               ├── stdio.rs
│   │               └── sse.rs
│   │
│   └── infra/                  # SQLite / Vector DB / EventBus / HTTP サーバ
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── db/
│           │   ├── mod.rs
│           │   ├── migrations/
│           │   └── models.rs
│           ├── event_bus.rs    # tokio::sync::broadcast ラッパ
│           ├── server/         # Axum (feature = "web-ui")
│           │   ├── mod.rs
│           │   ├── routes.rs
│           │   └── sse.rs
│           └── metrics.rs
│
└── docs/
    ├── architecture.md         # 本ドキュメント
    └── workflow/
        ├── agent.md
        ├── cli.md
        ├── config.md
        ├── discord.md
        ├── domain.md
        ├── infra.md
        ├── memory.md
        └── tools.md
```

---

## 3. クレート責務一覧

| クレート | 役割 | 主要依存 |
|---|---|---|
| `cli` | 引数解析・サブコマンド分岐・初回検知 | `clap`, `setup`, `config` |
| `setup` | 初回 TUI ウィザード・CLI フォールバック | `ratatui`, `crossterm`, `config` |
| `config` | 設定スキーマ定義・ロード・検証 | `serde`, `toml`, `dirs` |
| `discord` | Serenity イベント処理・コマンドルーティング | `serenity`, `agent`, `infra` |
| `agent` | Rig 推論ループ・セッション・コンテキスト管理 | `rig-core`, `memory`, `tools`, `infra` |
| `memory` | 短期・中期・長期の3層記憶管理・ベクトル検索 | `rig-core`, `qdrant-client`, `infra` |
| `tools` | ツール登録・実行・権限判定・MCP クライアント | `agent`, `infra` |
| `infra` | SQLite 永続化・EventBus・Axum HTTP | `sqlx`, `tokio`, `axum` (feature) |

---

## 4. 起動シーケンス

```
discord-agent run
      │
      ▼
[cli] 引数解析 (clap)
      │
      ▼
[config] ~/.config/discord-agent/config.toml 存在確認
      │
      ├─ 不在 ──▶ [setup] TUI ウィザード または CLI フォールバック
      │                │
      │                ▼ config.toml 書き出し
      │
      ▼
[config] 設定ロード・検証
      │
      ▼
[infra] SQLite 初期化・マイグレーション実行
      │
      ▼
[infra] EventBus 起動
      │
      ├─ feature: web-ui ──▶ [infra] Axum サーバ起動 (非同期)
      │
      ▼
[tools] ToolRegistry 初期化 (ビルトイン + MCP クライアント接続)
      │
      ▼
[memory] MemoryStore 初期化
      │   ├── 短期: DashMap 確保
      │   ├── 中期/長期: Vector DB (Qdrant) 接続確認・コレクション作成
      │   └── Embedding モデル初期化
      │
      ▼
[agent] SessionManager・ProviderAdapter 初期化
      │
      ▼
[discord] Serenity Client 起動・スラッシュコマンド登録
      │
      ▼
Discord WebSocket 接続完了 → イベント待機ループ
```

---

## 5. Entry / CLI 層

### 5.1 設計方針

`cli` クレートはビジネスロジックを一切持たず、**引数解析と各クレートへの委譲のみ**を担います。codex-rs の CLI 層と同じ「引数統合・モード別クレート呼び分け」アプローチを採用します。

### 5.2 サブコマンド一覧

| サブコマンド | 説明 |
|---|---|
| `run` | 通常起動。Discord Gateway を立ち上げてイベント待機 |
| `config` | 設定ファイルをエディタで開く、または TUI 再実行 |
| `config get <key>` | 設定値を標準出力に表示 |
| `config set <key> <value>` | CLI から設定値を上書き |
| `mcp-server` | MCP サーバモードで起動（外部ツールから Agent を呼び出す） |
| `serve` | HTTP API サーバのみ起動（feature: web-ui、将来対応） |

### 5.3 実装例

```rust
// crates/cli/src/main.rs
#[derive(Parser)]
#[command(name = "discord-agent", version)]
enum Cli {
    Run(RunArgs),
    Config(ConfigArgs),
    McpServer(McpServerArgs),
    #[cfg(feature = "web-ui")]
    Serve(ServeArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();

    match cli {
        Cli::Run(args) => commands::run::execute(args).await,
        Cli::Config(args) => commands::config::execute(args).await,
        Cli::McpServer(args) => commands::mcp_server::execute(args).await,
        #[cfg(feature = "web-ui")]
        Cli::Serve(args) => commands::serve::execute(args).await,
    }
}
```

---

## 6. Setup / 初回セットアップ層

### 6.1 設計方針

OpenClaw の初回設定フローを参考に、**config.toml 不在を唯一のトリガー**とします。`--skip-setup` フラグまたは環境変数 `DISCORD_AGENT_TOKEN` が存在する場合は TUI を省略します。

### 6.2 TUI ウィザード（ratatui）

ステップ形式で進む画面構成です。

```
┌─────────────────────────────────────────────┐
│  Discord AI Agent — セットアップ (1/4)       │
├─────────────────────────────────────────────┤
│                                             │
│  Discord Bot Token を入力してください:       │
│  ▶ [________________________]               │
│                                             │
│  Discordポータルで Bot を作成し、            │
│  TOKEN をコピーして貼り付けてください。       │
│                                             │
│  [Enter: 次へ]  [Esc: 終了]                 │
└─────────────────────────────────────────────┘
```

| ステップ | 入力内容 |
|---|---|
| 1. Discord Token | Bot Token（必須） |
| 2. AI プロバイダ | Anthropic / OpenAI / カスタム endpoint 選択 |
| 3. モデル選択 | プロバイダに応じたモデル一覧から選択 |
| 4. ツール許可 | web_search / code_exec の有効・無効 |

### 6.3 CLI フォールバック

TTY が存在しない環境（Docker・CI など）向けに、コマンドライン引数だけで設定を完結させられます。

```bash
discord-agent run \
  --token "Bot xxxx" \
  --provider anthropic \
  --model claude-sonnet-4-5 \
  --skip-setup
```

### 6.4 設定ファイル書き出し

セットアップ完了後、`~/.config/discord-agent/config.toml` を生成します。既存ファイルがある場合は差分マージ（既存値を優先）します。

---

## 7. Discord Gateway 層 (Serenity)

### 7.1 設計方針

Serenity の `EventHandler` トレイトを実装し、Discord WebSocket イベントを受け取ります。この層は「受信・正規化・ルーティング」のみを担い、**推論ロジックを一切持ちません**。

### 7.2 EventHandler

```rust
// crates/discord/src/handler.rs
#[async_trait]
impl EventHandler for AgentHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        // ボット自身のメッセージは無視
        if msg.author.bot { return; }

        // メンションまたは DM のみ処理
        if !is_addressed(&ctx, &msg).await { return; }

        // ContextResolver でセッションキーを決定
        let session_key = self.context_resolver.resolve(&msg);

        // typing indicator を開始
        let _ = msg.channel_id.broadcast_typing(&ctx.http).await;

        // Agent へ委譲
        let response = self.agent_runtime
            .submit(session_key, msg.content.clone())
            .await;

        // 応答を Discord へ送信（長文は自動分割）
        send_chunked(&ctx, &msg, response).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        self.command_router.dispatch(ctx, interaction).await;
    }
}
```

### 7.3 CommandRouter

スラッシュコマンドの登録と処理を管理します。

| コマンド | 説明 |
|---|---|
| `/ask <message>` | エージェントへメッセージを送信 |
| `/clear` | 現在のチャンネルのセッション履歴をリセット |
| `/history` | 直近の会話履歴を表示 |
| `/tools` | 有効なツール一覧を表示 |
| `/abort` | 実行中のエージェントタスクを中断 |

### 7.4 ContextResolver

Discord のコンテキスト情報を `SessionKey` に正規化します。

```rust
pub struct SessionKey {
    pub guild_id: Option<GuildId>,
    pub channel_id: ChannelId,
    pub thread_id: Option<ChannelId>,
    pub kind: SessionKind, // GuildChannel | Thread | DirectMessage
}
```

- **スレッド**: スレッド ID をキーにし、親チャンネルとは独立したセッションを持つ
- **DM**: ユーザーごとに独立したセッション
- **チャンネル**: チャンネル ID をキーに、サーバー横断で一意

---

## 8. Agent Core 層 (Rig)

### 8.1 設計方針

openclaude の `QueryEngine` に相当する層です。Rig の `Agent` / `Pipeline` 抽象を活用し、**推論ループ・ツール呼び出し・セッション管理・記憶統合**を担います。UI（Discord）とインフラ（SQLite・HTTP）には依存しません。記憶の読み書きは `memory` クレートの `MemoryStore` を通じて行います。

### 8.2 AgentRuntime

```rust
// crates/agent/src/runtime.rs
pub struct AgentRuntime {
    session_manager: Arc<SessionManager>,
    context_manager: Arc<ContextManager>,
    memory_store: Arc<MemoryStore>,        // 3層記憶への統合アクセス
    provider: Arc<dyn ProviderAdapter>,
    tool_registry: Arc<ToolRegistry>,
}

impl AgentRuntime {
    pub async fn submit(
        &self,
        session_key: SessionKey,
        user_input: String,
    ) -> AgentResponse {
        let session = self.session_manager.get_or_create(&session_key).await;

        // 1. 中期・長期記憶から関連情報を検索（semantic search）
        let recalled = self.memory_store.recall(&session_key, &user_input).await;

        // 2. コンテキスト構築（短期 + 想起した記憶を注入）
        let context = self.context_manager
            .build(&session, &user_input, &recalled)
            .await;

        // 3. Rig エージェント構築
        let agent = self.provider
            .build_agent()
            .preamble(context.system_prompt())
            .tools(self.tool_registry.enabled_tools(&session_key))
            .build();

        // 4. 推論ループ実行
        let result = agent.prompt(context.user_message()).await?;

        // 5. 短期記憶（セッション）を更新
        self.memory_store.push_short_term(&session_key, &user_input, &result).await;
        self.session_manager.append(&session_key, &user_input, &result).await;

        // 6. セッション終了判定 → 中期記憶へサマリーを昇格
        if self.memory_store.should_summarize(&session_key).await {
            self.memory_store.promote_to_mid_term(&session_key).await;
        }

        // 7. 重要情報を長期記憶へ抽出・保存
        self.memory_store.extract_long_term(&session_key, &result).await;

        result
    }
}
```

### 8.3 SessionManager

チャンネル（`SessionKey`）ごとに会話履歴を分離管理します。

```rust
pub struct Session {
    pub key: SessionKey,
    pub messages: Vec<ChatMessage>,  // Rig の Message 型
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub token_count: usize,
}
```

- セッションはメモリキャッシュ（`DashMap`）と SQLite の二層管理
- 非アクティブセッションは設定で指定した時間後に自動アーカイブ
- `fork` でセッションを分岐し、スレッドごとの文脈を保持

### 8.4 ContextManager

トークン上限への対処・システムプロンプトの組み立て・記憶から想起した情報の注入を担います。

```rust
pub struct ContextManager {
    max_tokens: usize,
    compaction_threshold: f32, // 例: 0.8 (80% 超で圧縮)
}

impl ContextManager {
    pub async fn build(
        &self,
        session: &Session,
        input: &str,
        recalled: &RecalledMemory,   // MemoryStore から想起された情報
    ) -> Context {
        let mut messages = session.messages.clone();

        // トークン数が閾値を超えた場合、古いメッセージをサマリーに置き換え
        if self.needs_compaction(&messages) {
            messages = self.compact(messages).await;
        }

        // システムプロンプトに想起した記憶ブロックを挿入
        let system_prompt = self.build_system_prompt_with_memory(session, recalled);

        Context {
            system_prompt,
            messages,
            user_message: input.to_string(),
        }
    }

    fn build_system_prompt_with_memory(
        &self,
        session: &Session,
        recalled: &RecalledMemory,
    ) -> String {
        let mut prompt = self.base_system_prompt();

        // 長期記憶ブロック（ユーザーや状況に関する重要な情報）
        if !recalled.long_term.is_empty() {
            prompt.push_str("\n\n## 記憶している重要な情報\n");
            for mem in &recalled.long_term {
                prompt.push_str(&format!("- {}\n", mem.content));
            }
        }

        // 中期記憶ブロック（過去の会話サマリー）
        if !recalled.mid_term.is_empty() {
            prompt.push_str("\n\n## 関連する過去の会話\n");
            for summary in &recalled.mid_term {
                prompt.push_str(&format!("- {}\n", summary.content));
            }
        }

        prompt
    }
}
```

### 8.5 ProviderAdapter

Rig のプロバイダ抽象を薄くラップし、設定ファイルで切り替え可能にします。

```rust
pub trait ProviderAdapter: Send + Sync {
    fn build_agent(&self) -> AgentBuilder;
    fn provider_name(&self) -> &str;
}

// Anthropic 実装
pub struct AnthropicProvider { client: rig::providers::anthropic::Client }

// OpenAI 実装
pub struct OpenAiProvider { client: rig::providers::openai::Client }
```

---

## 9. Memory 層（3層設計）

### 9.1 設計方針

人間の記憶モデルを参考に、**揮発性・保持期間・用途**の異なる3層で情報を管理します。ベクトル検索には Rig が提供するベクトルストア抽象と Qdrant を使用します。

```
┌─────────────────────────────────────────────────────────────────┐
│                         MemoryStore                              │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 短期記憶 (ShortTermMemory)                               │    │
│  │ ・現在セッションのメッセージ全文                         │    │
│  │ ・DashMap<SessionKey, Vec<Message>> でインメモリ保持     │    │
│  │ ・セッション終了または圧縮閾値到達で中期記憶へ昇格       │    │
│  └─────────────────────────────────────────────────────────┘    │
│                           ↓ 昇格 (promote)                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 中期記憶 (MidTermMemory)                                 │    │
│  │ ・過去セッションの会話サマリー                           │    │
│  │ ・Vector DB (Qdrant: collection = "mid_term") に格納     │    │
│  │ ・クエリとの類似度で上位 k 件を想起                      │    │
│  │ ・保持期間: 設定で指定（例: 30 日）                      │    │
│  └─────────────────────────────────────────────────────────┘    │
│                           ↓ 抽出 (extract)                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ 長期記憶 (LongTermMemory)                                │    │
│  │ ・ユーザーの好み・重要な事実・繰り返し登場する情報       │    │
│  │ ・Vector DB (Qdrant: collection = "long_term") に格納    │    │
│  │ ・モデルが重要と判断した情報を明示的に抽出・保存         │    │
│  │ ・保持期間: 無期限（手動削除のみ）                       │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 9.2 MemoryStore（統合インターフェース）

`agent` クレートから見えるインターフェースは `MemoryStore` の1つのみです。3層の詳細は隠蔽されます。

```rust
// crates/memory/src/store.rs
pub struct MemoryStore {
    short_term: ShortTermMemory,
    mid_term:   MidTermMemory,
    long_term:  LongTermMemory,
    embedder:   Arc<dyn Embedder>,
}

pub struct RecalledMemory {
    pub mid_term:  Vec<MemoryEntry>,  // 類似サマリー（上位 k 件）
    pub long_term: Vec<MemoryEntry>,  // 類似重要情報（上位 k 件）
}

pub struct MemoryEntry {
    pub content:    String,
    pub score:      f32,              // コサイン類似度
    pub created_at: DateTime<Utc>,
    pub metadata:   HashMap<String, String>,
}

impl MemoryStore {
    /// 現在のクエリに関連する中期・長期記憶を検索して返す
    pub async fn recall(
        &self,
        session_key: &SessionKey,
        query: &str,
    ) -> RecalledMemory {
        let embedding = self.embedder.embed(query).await;
        let (mid, long) = tokio::join!(
            self.mid_term.search(&embedding, session_key, TOP_K),
            self.long_term.search(&embedding, session_key, TOP_K),
        );
        RecalledMemory { mid_term: mid, long_term: long }
    }

    /// 短期記憶にメッセージを追加
    pub async fn push_short_term(
        &self,
        session_key: &SessionKey,
        user: &str,
        assistant: &str,
    ) { ... }

    /// 短期記憶のサマリーを生成して中期記憶へ書き込む
    pub async fn promote_to_mid_term(&self, session_key: &SessionKey) { ... }

    /// レスポンスから重要情報を抽出して長期記憶へ書き込む
    pub async fn extract_long_term(
        &self,
        session_key: &SessionKey,
        response: &str,
    ) { ... }

    /// 短期記憶の件数が閾値を超えたら中期記憶への昇格を促す
    pub async fn should_summarize(&self, session_key: &SessionKey) -> bool { ... }
}
```

### 9.3 短期記憶（ShortTermMemory）

```rust
// crates/memory/src/short_term.rs
pub struct ShortTermMemory {
    store: DashMap<SessionKey, VecDeque<ShortTermEntry>>,
    max_entries: usize,  // 例: 50 メッセージ
}

pub struct ShortTermEntry {
    pub role:       Role,   // User | Assistant | Tool
    pub content:    String,
    pub timestamp:  DateTime<Utc>,
    pub token_count: usize,
}
```

- セッション終了・圧縮閾値・`/clear` コマンドのいずれかで `promote_to_mid_term` がトリガーされます
- インメモリのみで SQLite には書き込みません（会話履歴の永続化は `SessionManager` の責務）

### 9.4 中期記憶（MidTermMemory）

```rust
// crates/memory/src/mid_term.rs
pub struct MidTermMemory {
    db: Arc<dyn VectorDbClient>,
    collection: String,       // "mid_term"
    retention_days: u32,      // 例: 30
}

impl MidTermMemory {
    /// 短期記憶のメッセージ群をサマリー化してベクトル保存
    pub async fn store_summary(
        &self,
        session_key: &SessionKey,
        messages: &[ShortTermEntry],
        summary: String,       // LLM が生成したサマリー文
        embedding: Vec<f32>,
    ) -> Result<()> {
        self.db.upsert(UpsertRequest {
            collection: &self.collection,
            id:         Uuid::new_v4().to_string(),
            vector:     embedding,
            payload: json!({
                "content":     summary,
                "guild_id":    session_key.guild_id,
                "channel_id":  session_key.channel_id.to_string(),
                "kind":        session_key.kind,
                "created_at":  Utc::now().timestamp(),
            }),
        }).await
    }

    /// クエリの埋め込みベクトルで類似サマリーを検索
    pub async fn search(
        &self,
        embedding: &[f32],
        session_key: &SessionKey,
        top_k: usize,
    ) -> Vec<MemoryEntry> { ... }
}
```

サマリーの生成は `AgentRuntime` が LLM を呼び出して行います。短期記憶のメッセージを渡し、「この会話の要点を3文で要約してください」のようなプロンプトでサマリーを生成します。

### 9.5 長期記憶（LongTermMemory）

```rust
// crates/memory/src/long_term.rs
pub struct LongTermMemory {
    db: Arc<dyn VectorDbClient>,
    collection: String,   // "long_term"
}

impl LongTermMemory {
    /// モデルの出力から重要情報を抽出してベクトル保存
    pub async fn store(
        &self,
        session_key: &SessionKey,
        fact: String,
        embedding: Vec<f32>,
        tags: Vec<String>,    // 例: ["preference", "user_name", "project"]
    ) -> Result<()> { ... }

    pub async fn search(
        &self,
        embedding: &[f32],
        session_key: &SessionKey,
        top_k: usize,
    ) -> Vec<MemoryEntry> { ... }

    /// 明示的な削除（ユーザーが /forget コマンドを使用した場合など）
    pub async fn delete(&self, id: &str) -> Result<()> { ... }
}
```

重要情報の抽出は、モデルの応答後に非同期で行います。抽出プロンプト例:

```
以下の応答から、将来の会話で参照すべき重要な情報があれば JSON で出力してください。
なければ空配列を返してください。

形式: [{"fact": "...", "tags": ["..."]}]

応答: {response}
```

### 9.6 Embedding

```rust
// crates/memory/src/embedding.rs
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Vec<f32>;
    fn dimension(&self) -> usize;
}

// Rig の EmbeddingModel を利用
pub struct RigEmbedder {
    model: Arc<dyn rig::embeddings::EmbeddingModel>,
}

// テスト用: ランダムベクトル（次元数のみ一致させる）
pub struct MockEmbedder { dim: usize }
```

### 9.7 VectorDbClient トレイト

```rust
// crates/memory/src/vector_db/mod.rs
pub trait VectorDbClient: Send + Sync {
    async fn upsert(&self, req: UpsertRequest) -> Result<()>;
    async fn search(&self, req: SearchRequest) -> Result<Vec<SearchResult>>;
    async fn delete(&self, collection: &str, id: &str) -> Result<()>;
    async fn ensure_collection(&self, name: &str, dim: usize) -> Result<()>;
}

// Qdrant 実装
pub struct QdrantClient { inner: qdrant_client::Qdrant }

// テスト用インメモリ実装
pub struct InMemoryVectorDb { ... }
```

Qdrant はローカル起動（Docker）でも Qdrant Cloud でも動作します。設定でエンドポイントを切り替えられます。

### 9.8 記憶操作に対応する Discord コマンド

| コマンド | 説明 |
|---|---|
| `/memory list` | 長期記憶の一覧を表示（ページネーション付き） |
| `/memory forget <id>` | 指定した長期記憶を削除 |
| `/memory clear-session` | 現在セッションの短期記憶をリセット |
| `/memory stats` | 各層の件数・使用量を表示 |

---

## 10. Tool System 層

### 9.1 設計方針

Rig の `Tool` トレイトをベースに、ビルトインツール・MCP ツール・カスタムツールを**同一のインターフェースで扱える**レジストリを構築します。opencode の `ToolRegistry + plugin hook` アーキテクチャを参考にしています。

### 9.2 ToolRegistry

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn RigTool>>,
    permission_guard: PermissionGuard,
}

impl ToolRegistry {
    /// セッションキーのコンテキストで使用可能なツールのみを返す
    pub fn enabled_tools(&self, session_key: &SessionKey) -> Vec<Arc<dyn RigTool>> {
        self.tools
            .values()
            .filter(|t| self.permission_guard.is_allowed(t.name(), session_key))
            .cloned()
            .collect()
    }
}
```

### 9.3 ビルトインツール

| ツール名 | 説明 | デフォルト有効 |
|---|---|---|
| `web_search` | DuckDuckGo / Brave Search API を使った検索 | ✅ |
| `code_exec` | sandboxed な Rust/Python/JS コード実行 | ❌ (要明示許可) |
| `read_file` | ボットサーバー上の許可ディレクトリ内ファイル読み込み | ❌ |
| `discord_search` | 同一サーバー内のメッセージ履歴検索 | ✅ |

### 9.4 MCP クライアント

```rust
// crates/tools/src/mcp/client.rs
pub struct McpClient {
    server_config: McpServerConfig,
    transport: Box<dyn McpTransport>,
    tools: Vec<McpToolDef>,
}

pub trait McpTransport: Send + Sync {
    async fn call(&self, tool: &str, args: Value) -> Result<Value>;
}

// stdio トランスポート (子プロセス起動)
pub struct StdioTransport { child: tokio::process::Child }

// SSE トランスポート (HTTP ベース)
pub struct SseTransport { url: Url, client: reqwest::Client }
```

MCP サーバーは `config.toml` の `[[mcp_servers]]` セクションで定義し、起動時に自動接続します。

```toml
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
transport = "stdio"

[[mcp_servers]]
name = "github"
url = "https://mcp.example.com/github/sse"
transport = "sse"
```

### 9.5 Permission Guard

```rust
pub struct PermissionGuard {
    config: PermissionConfig,
}

impl PermissionGuard {
    /// ツール実行の可否を判定する
    pub fn is_allowed(&self, tool_name: &str, session_key: &SessionKey) -> bool {
        // 1. グローバル deny リストを最優先
        if self.config.deny.contains(tool_name) { return false; }

        // 2. ツールごとの required_role を確認
        if let Some(required) = self.config.tool_roles.get(tool_name) {
            return session_key.has_role(required);
        }

        // 3. グローバル allow リスト
        self.config.allow.contains(tool_name) || self.config.allow_all
    }
}
```

### 9.6 AbortHandle

長時間実行タスクを `/abort` コマンドや timeout で中断します。

```rust
pub struct AbortHandle {
    handles: DashMap<SessionKey, tokio::task::AbortHandle>,
}

impl AbortHandle {
    pub fn register(&self, key: SessionKey, handle: tokio::task::AbortHandle) { ... }
    pub fn abort(&self, key: &SessionKey) -> bool { ... }  // true: 中断成功
}
```

---

## 11. Infrastructure 層

### 11.2 Vector DB (Qdrant)

記憶層が使用するベクトルデータベースです。`memory` クレートから直接アクセスし、`infra` 層は接続管理のみを担います。

#### コレクション設計

| コレクション名 | 用途 | 次元数 | 距離関数 |
|---|---|---|---|
| `mid_term` | 会話サマリーの埋め込み | モデル依存（例: 1536） | Cosine |
| `long_term` | 重要情報の埋め込み | モデル依存（例: 1536） | Cosine |

#### ペイロードフィールド（共通）

```json
{
  "content":    "記憶の本文テキスト",
  "guild_id":   "サーバーID (nullable)",
  "channel_id": "チャンネルID",
  "kind":       "guild | thread | dm",
  "created_at": 1700000000,
  "tags":       ["preference", "user_name"]
}
```

#### フィルタリング

検索時は `guild_id` + `channel_id` でフィルタし、関係のないサーバー・チャンネルの記憶を混入させません。

```rust
// guild_id と channel_id による絞り込み例
Filter::must([
    Condition::matches("guild_id",   session_key.guild_id_str()),
    Condition::matches("channel_id", session_key.channel_id_str()),
])
```

#### ローカル起動（開発用）

```yaml
# docker-compose.yml
services:
  qdrant:
    image: qdrant/qdrant:latest
    ports:
      - "6333:6333"
    volumes:
      - qdrant_data:/qdrant/storage
```

### 11.3 StorageDB (sqlx + SQLite)

#### スキーマ

```sql
-- セッション
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,   -- SessionKey の文字列表現
    guild_id    TEXT,
    channel_id  TEXT NOT NULL,
    thread_id   TEXT,
    kind        TEXT NOT NULL,      -- 'guild' | 'thread' | 'dm'
    created_at  INTEGER NOT NULL,
    last_active INTEGER NOT NULL,
    archived    INTEGER DEFAULT 0
);

-- メッセージ
CREATE TABLE messages (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id),
    role        TEXT NOT NULL,      -- 'user' | 'assistant' | 'tool'
    content     TEXT NOT NULL,
    tool_calls  TEXT,               -- JSON
    token_count INTEGER,
    created_at  INTEGER NOT NULL
);

-- ツール実行ログ
CREATE TABLE tool_executions (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id),
    tool_name   TEXT NOT NULL,
    args        TEXT NOT NULL,      -- JSON
    result      TEXT,               -- JSON
    duration_ms INTEGER,
    error       TEXT,
    created_at  INTEGER NOT NULL
);
```

### 11.4 EventBus

全層が `EventBus` を通じてイベントを発行・購読します。将来の Web UI は SSE エンドポイントでこのバスをそのまま購読します。

```rust
// crates/infra/src/event_bus.rs
#[derive(Clone, Debug)]
pub enum AgentEvent {
    MessageReceived      { session_key: SessionKey, content: String },
    ThinkingStarted      { session_key: SessionKey },
    ToolCalled           { session_key: SessionKey, tool: String, args: Value },
    ToolResult           { session_key: SessionKey, tool: String, result: Value },
    ResponseChunk        { session_key: SessionKey, chunk: String },
    ResponseCompleted    { session_key: SessionKey, full_response: String },
    // 記憶層イベント
    MemoryRecalled       { session_key: SessionKey, mid_count: usize, long_count: usize },
    MemoryPromoted       { session_key: SessionKey },  // 短期→中期
    MemoryExtracted      { session_key: SessionKey, fact: String },  // 長期記憶に保存
    ErrorOccurred        { session_key: SessionKey, error: String },
}

pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn publish(&self, event: AgentEvent) { ... }
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> { ... }
}
```

### 11.5 HttpServer (feature: web-ui)

`Cargo.toml` の feature フラグで制御します。

```toml
# crates/infra/Cargo.toml
[features]
default = []
web-ui = ["axum", "tower", "tower-http"]
```

有効化すると以下のエンドポイントが利用可能になります。

| エンドポイント | メソッド | 説明 |
|---|---|---|
| `/api/sessions` | GET | セッション一覧 |
| `/api/sessions/:id/messages` | GET | メッセージ履歴 |
| `/api/sessions/:id/messages` | POST | メッセージ送信 |
| `/api/events` | GET (SSE) | リアルタイムイベントストリーム |
| `/api/tools` | GET | 有効ツール一覧 |
| `/api/config` | GET / PATCH | 設定取得・更新 |
| `/api/memory/long-term` | GET | 長期記憶一覧 |
| `/api/memory/long-term/:id` | DELETE | 長期記憶の削除 |
| `/api/memory/mid-term` | GET | 中期記憶（サマリー）一覧 |

### 11.6 Observability

```rust
// crates/infra/src/metrics.rs
// tracing crate でのログ出力
tracing::info!(session_key = %key, tool = %tool_name, "tool executed");

// メトリクス (将来: prometheus 形式でエクスポート可能)
pub struct Metrics {
    pub messages_total: Counter,
    pub tool_calls_total: CounterVec,  // ラベル: tool_name
    pub response_latency: Histogram,
}
```

ログレベルは環境変数 `RUST_LOG` で制御します。

```bash
RUST_LOG=discord_agent=debug,serenity=warn ./discord-agent run
```

---

## 12. 設定スキーマ

`~/.config/discord-agent/config.toml` の完全なスキーマです。

```toml
# Discord 設定
[discord]
token = "Bot xxxx"                # 必須
application_id = 1234567890       # スラッシュコマンド登録に必要
prefix = "!"                      # オプション: テキストコマンドのプレフィックス

# AI プロバイダ設定
[provider]
name = "anthropic"                # "anthropic" | "openai" | "custom"
api_key = "sk-ant-xxxx"
model = "claude-sonnet-4-5"
max_tokens = 8192
temperature = 0.7

# カスタムエンドポイント (name = "custom" の場合)
# base_url = "https://your-endpoint.example.com/v1"

# セッション設定
[session]
max_history_messages = 50         # セッション内の最大メッセージ数
compaction_threshold = 0.8        # トークン使用率がこの値を超えたら圧縮
archive_after_minutes = 60        # 非アクティブ後のアーカイブまでの時間

# 記憶層設定
[memory]
short_term_max_entries = 50       # 短期記憶の最大メッセージ件数（超えると中期昇格）
mid_term_top_k = 3                # 想起する中期記憶の最大件数
long_term_top_k = 5               # 想起する長期記憶の最大件数
mid_term_retention_days = 30      # 中期記憶の保持期間（日）
summarize_prompt = ""             # サマリー生成プロンプト（空の場合はデフォルト使用）
extract_facts = true              # 長期記憶への自動抽出を有効にするか

# ベクトル DB (Qdrant) 設定
[memory.vector_db]
url = "http://localhost:6333"     # Qdrant エンドポイント
api_key = ""                      # Qdrant Cloud 使用時に設定
mid_term_collection = "mid_term"
long_term_collection = "long_term"

# 埋め込みモデル設定
[memory.embedding]
provider = "openai"               # "openai" | "anthropic" | "ollama"
model = "text-embedding-3-small"
dimension = 1536

# ツール権限設定
[tools]
allow_all = false
allow = ["web_search", "discord_search"]
deny = []

# ツールごとのロール制限
[tools.roles]
code_exec = "ADMINISTRATOR"
read_file = "MANAGE_MESSAGES"

# MCP サーバー設定
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
transport = "stdio"
enabled = true

# ログ設定
[log]
level = "info"                    # "trace" | "debug" | "info" | "warn" | "error"
file = "~/.config/discord-agent/discord-agent.log"

# Web UI 設定 (feature: web-ui 有効時のみ)
[server]
enabled = false
host = "127.0.0.1"
port = 8080
auth_token = ""                   # Bearer トークン認証 (空の場合は無効)
```

---

## 13. データフロー（エンドツーエンド）

Discord メッセージが届いてから応答が返るまでの完全なフローです。

```
1. Discord WebSocket
       │ message_create イベント
       ▼
2. [discord] handler.rs
       │ ボット宛メッセージか判定
       │ typing indicator 送信
       ▼
3. [discord] ContextResolver
       │ SessionKey { channel_id, thread_id, kind } を生成
       ▼
4. [agent] AgentRuntime.submit(session_key, user_input)
       │
       ├─▶ [agent] SessionManager.get_or_create(session_key)
       │         └── キャッシュミス時: [infra] SQLite から復元
       │
       ├─▶ [memory] MemoryStore.recall(session_key, user_input)  ← 記憶想起
       │         ├── user_input を埋め込みベクトルに変換 (Embedder)
       │         ├── 中期記憶 (Qdrant: mid_term) から類似サマリーを top_k 件取得
       │         ├── 長期記憶 (Qdrant: long_term) から類似情報を top_k 件取得
       │         └── RecalledMemory { mid_term, long_term } を返す
       │
       ├─▶ [infra] EventBus.publish(MemoryRecalled { mid_count, long_count })
       │
       ├─▶ [agent] ContextManager.build(session, user_input, recalled)
       │         ├── 短期記憶（現セッションのメッセージ）をベース
       │         ├── トークン数チェック → 必要なら古いメッセージを圧縮
       │         └── システムプロンプトに長期・中期記憶ブロックを注入
       │
       ├─▶ [infra] EventBus.publish(ThinkingStarted)
       │
       ├─▶ [agent] ProviderAdapter → Rig Agent 構築
       │         └── tools: ToolRegistry.enabled_tools(session_key)
       │
       ▼
5. Rig 推論ループ開始
       │
       ├─ モデル呼び出し (Anthropic / OpenAI API)
       │   ※ システムプロンプトに想起した記憶が含まれる
       │
       ├─ ツール呼び出し要求あり?
       │     │ YES
       │     ▼
       │  [tools] PermissionGuard.is_allowed(tool, session_key)
       │     │ 許可
       │     ▼
       │  [tools] ツール実行 (ビルトイン or MCP)
       │     │
       │     ├─▶ [infra] EventBus.publish(ToolCalled)
       │     ├─▶ [infra] SQLite: tool_executions に記録
       │     └─▶ [infra] EventBus.publish(ToolResult)
       │     │
       │     └─ 結果をモデルへフィードバック → ループ継続
       │
       └─ 最終応答生成
             │
             ▼
6. [memory] MemoryStore.push_short_term(session_key, user_input, response)
       │   ・短期記憶（DashMap）にターンを追加
       │
       ├─▶ should_summarize? (短期記憶件数 ≥ short_term_max_entries)
       │     │ YES
       │     ├─▶ LLM でサマリー生成
       │     ├─▶ [memory] MidTermMemory.store_summary → Qdrant (mid_term)
       │     ├─▶ [infra] EventBus.publish(MemoryPromoted)
       │     └─▶ 短期記憶をリセット
       │
       ├─▶ [memory] MemoryStore.extract_long_term(session_key, response)  (非同期)
       │     ├─▶ LLM で重要情報を JSON 抽出
       │     ├─▶ 抽出件数 > 0 の場合: Qdrant (long_term) に upsert
       │     └─▶ [infra] EventBus.publish(MemoryExtracted { fact })
       │
       ├─▶ [agent] SessionManager.append(session_key, user_input, response)
       │         ├── メモリキャッシュ更新
       │         └── [infra] SQLite: messages に保存
       │
       ├─▶ [infra] EventBus.publish(ResponseCompleted)
       │
       ▼
7. [discord] send_chunked(ctx, msg, response)
       │ 2000 文字超は自動分割して複数メッセージに送信
       ▼
8. Discord へ応答表示
```

---

## 14. セキュリティ・権限モデル

### 14.1 基本方針

「強いデフォルト + 明示的緩和」を原則とします。設定で許可していないツールは実行されません。

### 14.2 権限レイヤー

```
優先度（高）
  │
  ├─ 1. グローバル deny リスト (config.toml [tools].deny)
  │       どのユーザー・ロールであっても実行不可
  │
  ├─ 2. ツールごとの required_role (config.toml [tools.roles])
  │       指定ロール以上のユーザーのみ実行可
  │
  ├─ 3. グローバル allow リスト (config.toml [tools].allow)
  │       許可リストに含まれるツールは全ユーザーが実行可
  │
  └─ 4. allow_all フラグ (config.toml [tools].allow_all)
          true の場合、deny に含まれないすべてのツールを許可

優先度（低）
```

### 14.3 シークレット管理

- Discord Token・API キーは環境変数（`DISCORD_TOKEN`, `ANTHROPIC_API_KEY`）での上書きを優先します
- config.toml をバージョン管理に含めないよう `.gitignore` に追加します
- Web UI 有効時の API エンドポイントは Bearer トークン認証で保護します
- Qdrant の `api_key` も環境変数 `QDRANT_API_KEY` でオーバーライド可能です

### 14.4 記憶層のセキュリティ

- 記憶の検索・保存は必ず `SessionKey` の `guild_id` + `channel_id` フィルタを付けてアクセスし、異なるサーバー・チャンネルの記憶が漏れないように分離します
- `/memory forget` コマンドは Discord のロール制限を設けることを推奨します（デフォルト: `MANAGE_MESSAGES` 以上）
- 長期記憶への自動抽出（`extract_facts = true`）は個人情報を意図せず保存するリスクがあるため、必要に応じて無効化できます

### 14.5 サンドボックス

`code_exec` ツールはデフォルト無効です。有効化する場合は以下のいずれかで実行環境を隔離することを推奨します。

- Docker コンテナ内での実行
- `landlock` / `seccomp` による Linux サンドボックス
- Firecracker / Wasmtime による仮想化実行

---

## 15. Web UI 拡張戦略

### 15.1 feature フラグによる段階追加

Web UI は `feature = "web-ui"` フラグで完全に制御します。フラグなしのビルドでは Axum への依存も含め、一切コンパイルされません。

```bash
# Web UI なしでビルド（デフォルト）
cargo build --release

# Web UI ありでビルド
cargo build --release --features web-ui
```

### 15.2 拡張時に変更が必要なファイル

Web UI を追加する際に変更が必要な箇所は以下のみです。コア層（agent / memory / tools）への変更は不要です。

| ファイル | 変更内容 |
|---|---|
| `crates/infra/src/server/routes.rs` | API ルート追加（memory エンドポイントを含む） |
| `crates/infra/src/server/sse.rs` | EventBus を SSE でブリッジ（MemoryRecalled 等も配信） |
| `crates/cli/src/commands/serve.rs` | `serve` サブコマンド実装 |
| `Cargo.toml` | feature フラグ有効化 |

### 15.3 EventBus が橋渡しをする設計

EventBus はコア層・記憶層が最初から発行しているため、Web UI クライアントは SSE エンドポイントを購読するだけでリアルタイム更新を受け取れます。

```
Agent Core ──publish()──▶ EventBus ──subscribe()──▶ Axum SSE handler
Memory 層  ──publish()──▶              │                   │
                                        │                   ▼
                                        │            Web ブラウザ
                                        │            (EventSource API)
                                        │
                                        └── MemoryRecalled
                                            MemoryPromoted
                                            MemoryExtracted
```

---

## 16. ビルドと開発運用

### 16.1 justfile タスク

```bash
# フォーマット
just fmt

# 全テスト実行
just test

# リリースビルド
just build

# Web UI 付きビルド
just build-full

# スキーマ生成 (config.toml の JSON Schema)
just schema

# ローカル実行（環境変数から設定を読む）
just run

# Qdrant をローカル Docker で起動（記憶層の開発用）
just qdrant-up

# Qdrant を停止
just qdrant-down
```

### 16.2 環境変数オーバーライド

config.toml のすべての値は環境変数でオーバーライドできます。

```bash
DISCORD_TOKEN="Bot xxxx" \
ANTHROPIC_API_KEY="sk-ant-xxxx" \
QDRANT_API_KEY="" \
RUST_LOG="discord_agent=debug" \
./discord-agent run
```

### 16.3 依存クレート一覧

| 用途 | クレート |
|---|---|
| 非同期ランタイム | `tokio` |
| Discord SDK | `serenity` |
| AI エージェント | `rig-core` |
| TUI | `ratatui`, `crossterm` |
| CLI 引数解析 | `clap` |
| 設定ファイル | `serde`, `toml`, `dirs` |
| SQLite | `sqlx` (SQLite feature) |
| ベクトル DB | `qdrant-client` |
| HTTP サーバ (feature) | `axum`, `tower`, `tower-http` |
| HTTP クライアント | `reqwest` |
| エラーハンドリング | `anyhow`, `thiserror` |
| ログ・トレース | `tracing`, `tracing-subscriber` |
| 並行データ構造 | `dashmap` |
| 時刻 | `chrono` |
| UUID | `uuid` |
| JSON | `serde_json` |

---

## 17. 設計上のトレードオフ

### 採用した設計

| 決定 | 理由 |
|---|---|
| Cargo workspace 形式 | クレート単位でのテスト・依存管理が明確になり、将来の分割も容易 |
| EventBus を最初から実装 | Web UI 追加時にコア層を変更せずに済む。テスト時のモック差し込みも容易 |
| feature フラグで Web UI を制御 | 不要な依存をバイナリに含めず、最小構成での配布が可能 |
| SessionKey によるセッション分離 | チャンネル・スレッド・DM ごとに独立した会話文脈を維持できる |
| PermissionGuard をツール実行前に挿入 | 「モデルが要求したら即実行」を防ぎ、安全なデフォルト動作を保証する |
| 記憶層を独立クレート (`memory`) に分離 | Agent Core から実装詳細（Qdrant・埋め込み）を隠蔽し、テスト時は `InMemoryVectorDb` に差し替えられる |
| VectorDbClient トレイトで DB 抽象化 | Qdrant を他ベクトル DB（Milvus・Weaviate 等）に切り替えても上位層を変更不要 |
| 長期記憶の抽出を非同期で実行 | 応答のレイテンシに影響しない。失敗しても会話は継続できる |

### 留意点

| 留意点 | 対処方針 |
|---|---|
| in-process でのツール実行 | `code_exec` 等リスクの高いツールはデフォルト無効、サンドボックス化を推奨 |
| Serenity の非同期モデル | Gateway イベントは Tokio タスクとして並行処理されるため、共有状態は `Arc<Mutex<>>` または `DashMap` で管理 |
| トークン上限への到達 | ContextManager の compaction が機能するよう、閾値設定を適切に行う必要がある |
| MCP サーバーの信頼境界 | MCP サーバーは外部プロセスであるため、接続先は明示的に設定ファイルへ列挙する |
| Qdrant の外部依存 | 開発時は Docker でローカル起動、本番は Qdrant Cloud または自己ホストを推奨。`InMemoryVectorDb` でテストは完結できる |
| 長期記憶への自動抽出の精度 | LLM による抽出なので誤った情報が保存される可能性がある。`/memory list` で定期的に確認・削除できるようにする |
| 埋め込みモデルの次元数固定 | コレクション作成後にモデルを変更すると次元数不一致でエラーになる。変更時はコレクションの再作成が必要 |
