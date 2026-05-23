# `nekoui-agent` クレート ワークフロー

## 役割

`nekoui-agent` は、ユーザー入力を受け取り、LLM 推論、セッション管理、記憶連携、応答生成をまとめて担う中核レイヤーです。`discord` や `cli` のような入出力層に直接依存せず、`SessionKey` や `CallerContext` を受け取って応答本文を返します。Rig SDK の `ToolServerHandle` を介してツール実行も管理します。

## 主な構成

- `runtime.rs`: `AgentRuntime` の本体。起動初期化、推論ループ、ツール管理、短期/中期/長期記憶の連携、イベント・メトリクス通知を担当
- `context.rs`: システムプロンプトの構築、記憶の注入、`CallerContext` の反映、会話ターンの圧縮を担当
- `session.rs`: セッションの生成・更新・削除を行う `SessionManager` と会話ターン型を定義
- `provider.rs`: OpenAI 互換プロバイダ向けの Rig アダプタを提供

### 依存クレート

- `nekoui-config`: モデル設定や各種パラメータ
- `nekoui-domain`: `SessionKey`、`CallerContext`
- `nekoui-infra`: `EventBus`、`Metrics`、`WebUiAgent`
- `nekoui-memory`: `MemoryStore`、`ShortTermEntry`、`Role`、`RecalledMemory`

## 起動時ワークフロー: `AgentRuntime::new_with_progress`

初期化は進捗付きで行われ、CLI 側のプログレスバーに反映されます。

1. `SessionManager` を `Arc` で初期化
2. `.config/INSTRUCTION.md` を読み込み、システムプロンプトとして保持。存在しない場合は初期化失敗
3. `ContextManager` を生成し、`max_tokens=16384`、`compaction_threshold=0.7` を設定。`MemoryStore` も `Arc` で保持
4. 会話用と要約用の 2 系統の `OpenAICompatibleAdapter` を初期化し、モデル名やパラメータを設定可能にする
5. 必要な進捗コールバックを設定
6. `ToolServer` を起動し、`ToolServerHandle` を保持

追加で、バックグラウンド処理のために次も初期化されます。

- 長期記憶抽出タスクを送る `mpsc` チャネル
- `tokio::spawn` で動く `extraction_task_processor`
- 同時実行数を制限する `Semaphore`
- 会話蓄積用の `DashMap`
- `EventBus` と `Metrics`
- 要約の二重実行を防ぐ `summarizing` 管理

`new()` は `new_with_progress` を空のコールバックで呼び出す簡易版です。

## ツール管理ワークフロー

`AgentRuntime` は `ToolServerHandle` を内部に保持し、外部からツールを動的に登録できます。

### ツール登録: `add_tool`

```rust
pub async fn add_tool(&self, tool: impl ToolDyn + 'static)
```

- ツールは `InstrumentedTool` でラップして `ToolServer` に登録
- 主な呼び出し元は `nekoui-discord` の起動時処理
- 登録対象は Discord API 連携ツール、設定で有効化される Web 検索/コード実行/ファイル読み取り系ツール、MCP ツールなど

### ツール実行

1. Rig エージェントが推論中にツール呼び出しを返す
2. `ToolServerHandle` が登録済みツールを検索
3. 該当ツールの `call(args)` を実行
4. 結果が LLM の応答に組み込まれる

## 推論ワークフロー: `submit`

`submit(session_key, user_id, user_input) -> Result<AgentResponse>` の流れは次の通りです。

1. `SessionManager` から `SessionKey` 単位のセッションを取得。なければ新規作成
2. `MemoryStore::recall` で中期・長期記憶を検索
3. `ContextManager::build` でプロンプトコンテキストを構築し、`caller_user_id` や `caller_guild_id` を注入
4. `OpenAICompatibleAdapter` で Rig エージェントを生成し、会話モデルを使用
5. 既存ターンを `chat_history` に変換
6. `agent.prompt(user_message, chat_history, max_tokens)` を実行。失敗時は指数バックオフ + jitter で再試行
7. 応答を短期記憶へ追記
8. `should_summarize` が `true` かつ同一セッションの要約が未実行なら、中期記憶への要約処理を実行
9. セッション履歴へ追記
10. メッセージ数を加算し、`long_term_extraction_interval` に達したら長期抽出をキューへ積む
11. `AgentResponse { content }` を返却

### プロンプト構成

- **System**: ベースシステムプロンプトに、`<ImportantMemories>` と `<PastConversations>` を使って記憶を注入し、`CallerContext` を埋め込む
- **Chat history**: 圧縮済みの過去ターンを `Vec<Message>` として渡す
- **Current message**: 最新のユーザー入力

## 中期記憶への要約ワークフロー

`promote_short_term_to_mid_term` の処理です。

1. 対象セッションの短期メッセージ一覧を取得
2. 空なら `Ok(())` を返して終了
3. `format_short_term_messages` で整形
4. 会話モデルとは別の要約モデルで要約を生成
5. `MemoryStore::promote_to_mid_term` で保存
6. 短期記憶はクリアされず、後続会話で再利用される

### トリガー

- **圧縮閾値到達時**: `submit` の途中で `should_summarize` が `true` の場合に即実行
- **`/clear` 実行時**: `clear_session` 内で短期メッセージを非同期に要約し、その後 `clear_short_term` と `SessionManager::clear` を実行

## 長期記憶抽出ワークフロー

抽出はバックグラウンドの `extraction_task_processor` で処理されます。

### 流れ

1. `submit` 完了後に `message_since_last_extraction` を加算
2. メッセージ数が `long_term_extraction_interval` に達したら、蓄積会話を取得してカウンタをリセット
3. `spawn_long_term_extraction` で `ExtractionTask { session_key, user_id, conversation_batch }` を `mpsc` チャネルへ送信
4. キューが満杯なら `warn` を出してタスクを破棄

### ワーカー

5. `mpsc` チャネルからタスクを受信
6. `Semaphore` で同時実行数を最大 3 に制限
7. 各タスクを `tokio::spawn` で実行

### 抽出処理

8. 会話バッチから JSON 配列の事実を取り出す専用プロンプトを要約モデルへ送信
9. 応答を `Vec<ExtractedFact>` としてパース
10. パース失敗時は本文中の `[` ... `]` 部分を再試行
11. それでも失敗する場合は `tokio_retry` で 1 回だけ再試行
12. 抽出結果が空でなければ `MemoryStore::extract_long_term` で保存

保存されるデータは `(fact, tags)` と `user_id` です。

## Web UI 連携

`AgentRuntime` は `nekoui-infra` の `WebUiAgent` trait を実装し、`event_bus()`、`metrics()`、`list_sessions()`、`submit()` を提供します。Web UI 機能は `feature = "web-ui"` で制御されます。

## セッション操作ワークフロー

- `get_history`: `SessionManager::get` でセッションを取得し、クローンして返す
- `clear_session`:
  1. 短期メッセージを取得
  2. 空でなければ `tokio::spawn` で `generate_mid_term_summary` と `promote_to_mid_term` を実行
  3. `MemoryStore::clear_short_term` で短期記憶を削除
  4. `SessionManager::clear` でセッションを削除
- `shutdown`:
  1. `extraction_tx` を drop
  2. 少し待機して、実行中のタスクが終わるのを待つ

## エラー時の挙動

- モデル呼び出しや保存に失敗した場合は `Result::Err` を返却
- 中期保存や長期抽出は失敗しても本体応答を継続し、`warn` で通知
- `.config/INSTRUCTION.md` がない場合は初期化失敗
- 長期記憶抽出の JSON パース失敗時はリトライし、それでも失敗した場合は `warn`
- 抽出キューが満杯の場合はタスクを破棄して `warn`
- ツール登録失敗時は `warn`
- 推論は指数バックオフ + jitter で複数回リトライ

## 連携ポイント

- 入力元: `nekoui-discord` の `/ask` コマンド
- 設定: `nekoui-config` のモデル/API/パラメータ設定
- 記憶: `nekoui-memory`
- ツール: `nekoui-tools`
- 型: `nekoui-domain::agent::session::SessionKey`、`CallerContext`
- 基盤: `nekoui-infra` の `EventBus`、`Metrics`、`WebUiAgent`
- ツール実行: Rig の `ToolServerHandle` と `InstrumentedTool`
