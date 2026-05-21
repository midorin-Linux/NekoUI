# `nekoai-agent` クレートのワークフロー

## 役割

`nekoai-agent` は、ユーザー入力を受けて LLM 推論を実行し、セッション管理・記憶連携・応答生成を行う中核レイヤーです。Discord や CLI などの入出力層には依存せず、`SessionKey`・`Option<String>`（user_id）・文字列入力を受けて応答文字列を返します。Rig SDK の `ToolServerHandle` を介してツール実行を管理します。

## 主な構成

- `runtime.rs` (827行): 起動初期化、推論ループ (AgentRuntime)、ツール管理（InstrumentedTool ラッパー）、要約/長期記憶抽出トリガー、抽出タスクプロセッサ、EventBus/Metrics 連携
- `context.rs` (139行): システムプロンプト構築（記憶注入・CallerContext 置換）と会話ターン圧縮
- `session.rs` (122行): セッションの生成・更新・削除（SessionManager, ConversationTurn）
- `provider.rs` (36行): OpenAI 互換の Rig `AgentBuilder` を組み立てる `OpenAICompatibleAdapter`

### 依存関係

- `nekoai-config`: Parameters（モデルパラメータ）
- `nekoai-domain`: SessionKey, CallerContext
- `nekoai-infra`: EventBus, Metrics, WebUiAgent trait
- `nekoai-memory`: MemoryStore, ShortTermEntry, Role, RecalledMemory

## 起動時ワークフロー（`AgentRuntime::new_with_progress`）

合計 6 ステップの進捗 (`RuntimeInitProgress`) を返し、CLI 側のプログレスバーに反映されます（ただし step 5 の進捗コールバックはスキップされる）。

1. `SessionManager` を `Arc` で初期化
2. `.config/INSTRUCTION.md` を読み込み、システム指示として保持（なければ初期化失敗）
3. `ContextManager` を生成（`max_tokens=16384`, `compaction_threshold=0.7`）。`MemoryStore` を `Arc` でラップ
4. 会話モデル + 要約モデルの 2 系統の `OpenAICompatibleAdapter` を初期化（別々のモデル名・パラメータを設定可能）
5. （コールバックなし - スキップ）
6. `ToolServer` を起動し `ToolServerHandle` を保持 → 進捗報告（2回コールされる）

**内部で実行される追加の初期化**:
- mpsc チャネル（容量 100）を作成し、長期記憶抽出タスクを送信する `extraction_tx` を保持
- `tokio::spawn` で `extraction_task_processor` をバックグラウンド起動
- 同時実行制限用の `Semaphore`（最大 3）
- `accumulated_conversations` / `message_since_last_extraction` の DashMap を初期化
- `EventBus` / `Metrics` を初期化
- 要約の同時実行防止用 `summarizing` DashMap

`new()` は `new_with_progress` を空のコールバックで呼び出す簡易ラッパー。

## ツール管理ワークフロー

`AgentRuntime` は `ToolServerHandle` を内部に保持し、外部からツールを動的に登録できます。

### ツール登録（`add_tool`）

```rust
pub async fn add_tool(&self, tool: impl ToolDyn + 'static)
```

- ツールは `InstrumentedTool` でラップされて `ToolServer` に登録される
- 呼び出し元: `nekoai-discord::client.rs`（起動時）
- 登録されるツール: 57 個以上の Discord API 連携ツール（`ToolAccess::Public`）+ Web検索ツール（config-gated）+ MCP ツール

### ツール実行

1. Rig エージェントが推論中にツール呼び出しを返す
2. `ToolServerHandle` が登録済みツールを検索
3. 該当ツールが `call(args)` を実行
4. 結果が LLM 応答に組み込まれる

## 推論ワークフロー（`submit`）

`submit(session_key, user_id, user_input) -> Result<AgentResponse>`:

1. `SessionManager` から `SessionKey` 単位でセッション取得（なければ新規作成）
2. `MemoryStore::recall` で中期/長期記憶を検索
3. `ContextManager::build` でプロンプトコンテキストを構築（`caller_user_id`, `caller_guild_id` を注入）
4. `OpenAICompatibleAdapter` で Rig エージェントを生成（会話モデルを使用）
5. コンテキストの既存ターンを `chat_history` に変換
6. `agent.prompt(user_message, chat_history, max_tokens)` を実行（5回リトライ、指数バックオフ + jitter、最大20ターン）
7. 短期記憶へ追記（`push_short_term`）
8. `should_summarize` が true かつ同一セッションの要約中でなければ中期記憶への昇格処理を実行
9. セッション履歴へ追記（`SessionManager::append`）
10. 蓄積メッセージ数をインクリメントし、`long_term_extraction_interval` に達したらバッチ抽出をキューイング
11. `AgentResponse { content }` を返却

**プロンプト構成**:
- **System** (`preamble`): ベースシステムプロンプト + 注入された記憶（`<ImportantMemories>` / `<PastConversations>` タグ）+ CallerContext プレースホルダ置換
- **Chat history** (`chat_history`): 圧縮済みの過去ターンを `Vec<Message>` として渡す
- **Current message**: 最新のユーザー入力

## 中期記憶昇格ワークフロー

`promote_short_term_to_mid_term` の動作:

1. 対象セッションの短期メッセージ一覧を取得
2. 空なら即座に `Ok(())` を返してスキップ
3. `format_short_term_messages` でメッセージを整形
4. **要約モデル**（会話モデルとは別）で 5-10 文の要約を生成（`generate_mid_term_summary`）
5. `MemoryStore::promote_to_mid_term` で要約を保存
6. 短期記憶はクリアしない（後続の会話で再利用される）

### トリガー

- **圧縮閾値到達時**: `submit` の途中で `should_summarize` が true の場合に即時実行
- **`/clear` 実行時（`clear_session`）**: 短期メッセージを `tokio::spawn` で非同期に昇格後、`clear_short_term` + `SessionManager::clear` を実行

## 長期記憶抽出ワークフロー

抽出はバックグラウンドの `extraction_task_processor` で処理されます。

**蓄積**:
1. `submit` 完了後、`message_since_last_extraction` をインクリメント
2. 蓄積メッセージが `long_term_extraction_interval` に達した場合、蓄積会話を取得しカウンタをリセット
3. `spawn_long_term_extraction` で `ExtractionTask { session_key, user_id, conversation_batch }` を mpsc チャネルに `try_send`
4. キューが満杯の場合は `warn` ログを出力しタスクを破棄

**非同期ワーカー（`extraction_task_processor`）**:
5. mpsc チャネルからタスクを受信
6. `Semaphore` で同時実行数を最大 3 に制限
7. 各タスクを `tokio::spawn` で非同期実行

**抽出処理（`extract_and_store_long_term_facts`）**:
8. 会話バッチから JSON 配列を抽出するための専用プロンプトを**要約モデル**に送信
9. 応答を `Vec<ExtractedFact>` としてパース
10. パース失敗時は文字列中の `[`...`]` 部分で再試行（`parse_extracted_facts`）
11. パースに失敗した場合、`tokio_retry`（最大 1 回リトライ、1 秒間隔）で再実行
12. 空でなければ `MemoryStore::extract_long_term` で保存

保存データは `(fact, tags)` と `user_id` です。

## WebUiAgent 連携

`AgentRuntime` は `nekoai-infra` の `WebUiAgent` trait を実装しており、`event_bus()`, `metrics()`, `list_sessions()`, `submit()` を提供します。Web UI 機能は `feature = "web-ui"` で制御されます。

## セッション操作ワークフロー

- `get_history`: `SessionManager::get` でセッションを取得しクローンして返す
- `clear_session`:
  1. 短期メッセージを取得
  2. 空でなければ `tokio::spawn` で非同期に `generate_mid_term_summary` → `promote_to_mid_term` を実行
  3. `MemoryStore::clear_short_term` で短期記憶を削除
  4. `SessionManager::clear` でセッションを削除
- `shutdown`:
  1. `extraction_tx`（mpsc Sender）を drop
  2. 2 秒待機して処理中のタスク完了を待つ

## エラー時の挙動

- モデル呼び出しや保存で失敗した場合は `Result::Err` を返却
- 中期昇格/長期抽出は失敗しても本体応答は継続し `warn` ログで通知
- `.config/INSTRUCTION.md` がない場合は初期化失敗
- 長期記憶抽出の JSON パース失敗時は `tokio_retry`（最大 1 回）で再試行、それでも失敗した場合は `warn` ログ
- 抽出キューが満杯の場合はタスクを破棄し `warn` ログ
- ツール登録失敗時は `warn` ログ
- 推論は指数バックオフ + jitter で最大 5 回リトライ（100ms ベース、10s 最大）

## 連携ポイント

- 入力: `nekoai-discord`（`ask` コマンド）
- 設定: `nekoai-config`（2系統モデル/API/パラメータ）
- 記憶: `nekoai-memory`
- ツール: `nekoai-tools`（Discord + Web検索 + MCP ツール群）
- 型: `nekoai-domain::agent::session::SessionKey` / `CallerContext`
- 基盤: `nekoai-infra`（EventBus, Metrics, WebUiAgent）
- ツール実行: Rig `ToolServerHandle` + `InstrumentedTool` ラッパー
