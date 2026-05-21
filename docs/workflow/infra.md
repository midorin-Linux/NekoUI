# `nekoai-infra` クレートのワークフロー

## 役割

`nekoai-infra` は横断的な基盤機能を提供します。ロギング、イベントバス、メトリクス、Web UI サーバーを含みます。

## 主な構成

- `logging.rs` (127行): ファイルベース tracing 初期化（日次ローテーション、フィールド値トランケーション）
- `event_bus.rs` (72行): publish/subscribe イベントシステム（`tokio::sync::broadcast`）
- `metrics.rs` (88行): Prometheus 形式メトリクス収集
- `web_ui_agent.rs` (16行): Web UI 向け Agent インターフェース trait
- `http_server.rs` (135行): Axum HTTP サーバー（`feature = "web-ui"` で有効化、SSE + Prometheus metrics）
- `lib.rs` (7行): モジュール宣言（`http_server` は feature-gated）

## ログ初期化ワークフロー（`init_tracing`）

1. `logs` ディレクトリの存在確認、なければ作成
2. 日次ローテーションのファイルアペンダー `logs/nekoai.log` を設定（`tracing_appender::non_blocking`）
3. `.env` を読み込み（`dotenvy::dotenv()`）
4. `LOG_LEVEL` 環境変数から `EnvFilter` を構築（未設定時は `info`）
5. `tracing_subscriber::fmt()` を設定:
   - writer: non-blocking ファイル出力
   - env filter: `LOG_LEVEL`
   - ANSI: 無効（ファイルログ向け）
   - event format: カスタム `TruncatingEventFormat`（フィールド値 100 文字でトランケーション）
6. `WorkerGuard` を返却（drop 時にバッファフラッシュ）

ログ出力形式: `"YYYY-MM-DD HH:MM:SS LEVEL target field1=val1 field2=val2"`

## イベントバスワークフロー（`EventBus`）

`tokio::sync::broadcast` ベースの publish/subscribe。

### `AgentEvent` バリアント（10種類）

| イベント | 説明 |
|---|---|
| `MessageReceived { session_key, content }` | ユーザーメッセージ受信 |
| `ThinkingStarted { session_key }` | 推論開始 |
| `ToolCalled { session_key, tool, args }` | ツール呼び出し |
| `ToolResult { session_key, tool, result }` | ツール実行結果 |
| `ResponseChunk { session_key, chunk }` | 応答チャンク（ストリーミング） |
| `ResponseCompleted { session_key, full_response }` | 応答完了 |
| `MemoryRecalled { session_key, mid_count, long_count }` | 記憶想起 |
| `MemoryPromoted { session_key }` | 中期記憶昇格 |
| `MemoryExtracted { session_key, fact }` | 長期記憶抽出 |
| `ErrorOccurred { session_key, error }` | エラー発生 |

### メソッド

- `new(capacity: usize)`: 指定容量のブロードキャストチャネルを作成
- `publish(event)`: イベントを配信（購読者なし時は debug ログ）
- `subscribe()`: 新しい `broadcast::Receiver` を返却

## メトリクスワークフロー（`Metrics`）

### 収集項目（アトミックカウンタ + DashMap）

| メトリクス | 型 | 説明 |
|---|---|---|
| `messages_total` | `AtomicU64` | 全メッセージ数 |
| `tool_calls_total` | `DashMap<String, AtomicU64>` | ツール別呼び出し回数 |
| `response_latencies` | `Mutex<Vec<f64>>` | 応答レイテンシ（最大 1000 エントリのスライディングウィンドウ） |
| `start_time` | `Instant` | 起動時刻 |

### メソッド

- `new()` / `Default`: 初期化
- `record_message()`: メッセージカウント増加
- `record_tool_call(name)`: ツール呼び出しカウント増加
- `record_latency(duration)`: レイテンシ記録（1000 超で古いものを削除）
- `collect_prometheus()`: Prometheus テキスト形式で出力

### Prometheus 出力項目

- `nekoai_messages_total` (counter)
- `nekoai_tool_calls_total{tool="..."}` (counter)
- `nekoai_response_latency_seconds` (gauge, 最新値)
- `nekoai_uptime_seconds` (counter)

## WebUiAgent トレイト

```rust
#[async_trait]
pub trait WebUiAgent: Send + Sync {
    fn event_bus(&self) -> &EventBus;
    fn metrics(&self) -> &Metrics;
    async fn list_sessions(&self) -> Vec<SessionKey>;
    async fn submit(&self, session_key: SessionKey, user_id: Option<String>, content: String) -> anyhow::Result<String>;
}
```

`AgentRuntime` がこのトレイトを実装し、Web UI との統合を提供。

## HTTP サーバーワークフロー（`feature = "web-ui"`）

Axum ベースの HTTP サーバー。

### ルート

| パス | メソッド | 説明 |
|---|---|---|
| `GET /api/events` | SSE | `AgentEvent` の JSON ストリーム（15秒 keep-alive） |
| `GET /api/metrics` | GET | Prometheus テキスト形式メトリクス |

### セキュリティ

- **CORS**: `allowed_origins` が空の場合はループバック（127.0.0.1, localhost）のみ許可、それ以外は明示リスト
- **認証**: `auth_token` が設定されている場合、`Authorization: Bearer <token>` ヘッダーを検証（不一致は 401）

## 連携ポイント

- `nekoai-cli`: `init_tracing` を呼び出し、`WorkerGuard` を保持
- `nekoai-agent`: `EventBus` + `Metrics` + `WebUiAgent` を実装
