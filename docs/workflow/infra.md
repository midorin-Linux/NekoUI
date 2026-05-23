# `nekoui-infra` クレート ワークフロー

## 役割

`nekoui-infra` は横断的な基盤機能を提供します。ロギング、イベントバス、メトリクス、Web UI 用 HTTP サーバーを含みます。

## 主な構成

- `logging.rs`: ファイルベースの tracing 初期化。日次ローテーションやフィールド値の切り詰めを担当
- `event_bus.rs`: `tokio::sync::broadcast` を使った publish / subscribe のイベントシステム
- `metrics.rs`: Prometheus 形式のメトリクス収集
- `web_ui_agent.rs`: Web UI 向けの `Agent` インターフェース
- `http_server.rs`: Axum ベースの HTTP サーバー。`feature = "web-ui"` で有効化され、SSE と Prometheus metrics を提供
- `lib.rs`: モジュール宣言。`http_server` は feature-gated

## ログ初期化ワークフロー: `init_tracing`

1. `logs` ディレクトリの存在を確認し、なければ作成
2. 日次ローテーション付きのファイルアペンダー `logs/nekoui.log` を設定
3. `.env` を読み込む
4. `LOG_LEVEL` 環境変数から `EnvFilter` を構築。未設定時は `info`
5. `tracing_subscriber::fmt()` を設定
   - writer: non-blocking のファイル出力
   - env filter: `LOG_LEVEL`
   - ANSI: 無効
   - event format: `TruncatingEventFormat` を使用し、フィールド値を 100 文字で切り詰める
6. `WorkerGuard` を返却し、drop 時にバッファをフラッシュする

ログ出力形式は `YYYY-MM-DD HH:MM:SS LEVEL target field1=val1 field2=val2` です。

## イベントバスワークフロー: `EventBus`

`tokio::sync::broadcast` ベースの publish / subscribe を提供します。

### `AgentEvent` のバリアント

| イベント | 説明 |
|---|---|
| `MessageReceived { session_key, content }` | ユーザーメッセージ受信 |
| `ThinkingStarted { session_key }` | 推論開始 |
| `ToolCalled { session_key, tool, args }` | ツール呼び出し |
| `ToolResult { session_key, tool, result }` | ツール実行結果 |
| `ResponseChunk { session_key, chunk }` | 応答チャンクのストリーミング |
| `ResponseCompleted { session_key, full_response }` | 応答完了 |
| `MemoryRecalled { session_key, mid_count, long_count }` | 記憶想起 |
| `MemoryPromoted { session_key }` | 中期記憶への昇格 |
| `MemoryExtracted { session_key, fact }` | 長期記憶抽出 |
| `ErrorOccurred { session_key, error }` | エラー発生 |

### メソッド

- `new(capacity: usize)`: 指定容量のブロードキャストチャネルを作成
- `publish(event)`: イベントを配信。購読者がいない場合は debug ログ
- `subscribe()`: 新しい `broadcast::Receiver` を返す

## メトリクスワークフロー: `Metrics`

### 収集項目

| メトリクス | 型 | 説明 |
|---|---|---|
| `messages_total` | `AtomicU64` | 総メッセージ数 |
| `tool_calls_total` | `DashMap<String, AtomicU64>` | ツール別の呼び出し回数 |
| `response_latencies` | `Mutex<Vec<f64>>` | 応答レイテンシ。最大 1000 件のスライディングウィンドウ |
| `start_time` | `Instant` | 起動時刻 |

### メソッド

- `new()` / `Default`: 初期化
- `record_message()`: メッセージカウントを増やす
- `record_tool_call(name)`: ツール呼び出しカウントを増やす
- `record_latency(duration)`: レイテンシを記録し、古いデータを削除
- `collect_prometheus()`: Prometheus テキスト形式で出力

### Prometheus 出力項目

- `nekoui_messages_total` (counter)
- `nekoui_tool_calls_total{tool="..."}` (counter)
- `nekoui_response_latency_seconds` (gauge、最新値)
- `nekoui_uptime_seconds` (counter)

## `WebUiAgent` トレイト

```rust
#[async_trait]
pub trait WebUiAgent: Send + Sync {
    fn event_bus(&self) -> &EventBus;
    fn metrics(&self) -> &Metrics;
    async fn list_sessions(&self) -> Vec<SessionKey>;
    async fn submit(&self, session_key: SessionKey, user_id: Option<String>, content: String) -> anyhow::Result<String>;
}
```

`AgentRuntime` がこのトレイトを実装し、Web UI との統合を提供します。

## HTTP サーバーワークフロー: `feature = "web-ui"`

Axum ベースの HTTP サーバーです。

### ルート

| パス | メソッド | 説明 |
|---|---|---|
| `GET /api/events` | SSE | `AgentEvent` の JSON ストリームを 5 秒 keep-alive 付きで配信 |
| `GET /api/metrics` | GET | Prometheus テキスト形式のメトリクスを返す |

### セキュリティ

- **CORS**: `allowed_origins` が空のときはループバックホストと localhost のみ許可し、それ以外は明示的なオリジンだけ許可
- **認証**: `auth_token` が設定されている場合は `Authorization: Bearer <token>` を検証し、不一致なら 401 を返す

## 連携ポイント

- `nekoui-cli`: `init_tracing` を呼び出し、`WorkerGuard` を保持
- `nekoui-agent`: `EventBus`、`Metrics`、`WebUiAgent` を利用
