# `nekoai-memory` クレートのワークフロー

## 役割

`nekoai-memory` は短期・中期・長期の 3 層記憶を統合管理し、検索（recall）と保存（promotion/extraction）を提供します。外部のベクトル DB と埋め込みモデルの差異は内部で吸収します。

## 主な構成

- `store.rs` (279行): 3 層統合インターフェース（`MemoryStore`）
- `short_term.rs` (84行): セッション内インメモリ記憶（`DashMap`、`Role::User/Assistant/Tool`）
- `mid_term.rs` (145行): 会話サマリー保存・検索・保持期間クリーンアップ
- `long_term.rs` (227行): 重要事実保存・検索・削除（`search_by_guild`, `search_by_user` 対応）
- `embedding.rs` (125行): 埋め込み生成（OpenAI 互換 + Mock フォールバック、5回リトライ）
- `vector_db/mod.rs` (58行): ベクトル DB 抽象インターフェース（`VectorDbClient` trait）
- `vector_db/qdrant.rs` (360行): Qdrant 実装（`session_scope_filter`、コサイン類似度）
- `vector_db/inmemory.rs` (233行): インメモリ実装（テスト用途、コサイン類似度 + フィルタ評価）

## 初期化ワークフロー（`MemoryStore::new` + `initialize`）

1. `&AppConfig` から設定を読み込み
2. Qdrant クライアントを初期化（URL/API key）
3. 埋め込みモデルを初期化:
   - 成功: `OpenAICompatibleEmbedder`（Rig SDK + 指数バックオフリトライ）
   - 失敗: `MockEmbedder` へフォールバック（`warn` ログ、FNV-1a ハッシュ + LCG 疑似乱数）
4. `MidTermMemory` / `LongTermMemory` を構築
5. `initialize().await` で両コレクションを `ensure_collection`

### テスト用コンストラクタ

`with_components(mid_term, long_term, embedder, short_term_max, mid_term_top_k, long_term_top_k)`: 既存コンポーネントを直接注入可能。

## 短期記憶ワークフロー（`ShortTermMemory`）

- `push_turn(session_key, user, assistant)`: 2 エントリ（User/Assistant）を同じタイムスタンプで追加、上限超過時は古いものから削除
- `get_messages(session_key)`: `Vec<ShortTermEntry>` を返却（各エントリは `role`, `content`, `timestamp`）
- `get_count(session_key)`: 現在のエントリ数
- `clear(session_key)`: セッション単位に削除
- Role: `User`, `Assistant`, `Tool` の 3 種類

## 想起ワークフロー（`MemoryStore::recall`）

1. `recall(session_key, query)` を呼び出し
2. `embedder.embed(query)` でクエリの埋め込みを生成
3. `tokio::join!` で中期/長期記憶を並行検索
4. `RecalledMemory { mid_term, long_term }` を返却

### `should_summarize` メソッド

短期記憶のエントリ数が `max_entry` に達したかを判定。

## 中期記憶ワークフロー

### `promote_to_mid_term`

1. 要約文を受け取り埋め込み化
2. `mid_term` コレクションへ upsert
3. 短期記憶をクリア

### `promote_to_mid_term_with_messages`

同上だが、メッセージを外部から受け取る（`clear_session` 時に使用）。

### 保存される payload 構造

- `content`: 要約文
- `guild_id`, `channel_id`, `kind`, `created_at`, `message_count`

### 検索

- `search(session_key, query, top_k)`: セッションスコープで検索（`session_scope_filter` 適用）
- `search_with_embedding(session_key, embedding, top_k)`: プリエンベッド済み検索

### 保持期間クリーンアップ

`MemoryStore::start_cleanup_job()` で 24 時間ごとに `delete_old_entries()` を実行し `created_at < cutoff` のデータを削除。

## 長期記憶ワークフロー

### `extract_long_term`

1. `facts: Vec<(String, Vec<String>)>`（事実, タグ）を受け取り
2. 各 fact を埋め込み化
3. `long_term` コレクションへ upsert

### 保存される payload 構造

- `content`: 事実
- `guild_id`, `channel_id`, `kind`, `created_at`, `tags`, `user_id`（Option）

### 検索

- `search(session_key, query, top_k)`: セッションスコープ
- `search_by_guild(guild_id, query, top_k)`: ギルド全体
- `search_by_user(user_id, query, top_k)`: ユーザー固有
- `search_with_embedding(session_key, embedding, top_k)`: プリエンベッド済み

### 削除

- `delete(id)`: ID 指定削除
- `delete_by_channel(channel_id)`: チャンネル単位削除

長期記憶は自動期限削除なし、明示削除のみ。

## ベクトル DB ワークフロー

`VectorDbClient` trait の操作:
- `upsert(request)`: ベクトル + ペイロード保存
- `search(request)`: ベクトル検索（フィルタ + top_k）
- `delete(collection, id)`: ID 削除
- `delete_by_filter(collection, filter)`: フィルタ削除
- `ensure_collection(name, dim)`: コレクション作成/確認

### Qdrant 実装

- Qdrant ネイティブ `Filter` / `Condition` に変換
- `session_scope_filter`: `guild_id` + `channel_id` + `kind` でフィルタリング
- リトライ戦略: 指数バックオフ（100ms ベース、10s 最大、jitter、5回）
- `SearchPointsBuilder` を使用

### InMemory 実装

- コサイン類似度（事前計算済みノルム）でランキング
- `must`/`should` 条件をローカル評価
- `Default` trait 実装

## 埋め込みワークフロー

### `OpenAICompatibleEmbedder`

- Rig SDK の `openai::EmbeddingModel` をラップ
- 5 回リトライ（指数バックオフ + jitter）
- 全リトライ失敗時は `MockEmbedder` にフォールバック
- `f64` ベクトルを `Vec<f32>` にキャスト

### `MockEmbedder`

- FNV-1a ハッシュで stable seed 生成
- LCG 疑似乱数でベクトル生成
- テストや API 失敗時のフォールバック

## ヘルパー関数

### `search_result_to_entry`（`long_term.rs` 内 `pub(crate)`）

`SearchResult` → `MemoryEntry` 変換:
- `content`: payload から
- `score`: 検索スコア
- `created_at`: Unix タイムスタンプ → `DateTime<Utc>`
- `metadata`: payload 全体

## 連携ポイント

- `nekoai-agent`: `recall`, `promote_to_mid_term`, `extract_long_term`, `push_short_term`, `should_summarize`
- `nekoai-config`: 記憶設定と接続先
- `nekoai-domain`: `SessionKey` スコープ
