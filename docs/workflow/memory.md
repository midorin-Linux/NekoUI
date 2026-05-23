# `nekoui-memory` クレート ワークフロー

## 役割

`nekoui-memory` は、短期・中期・長期の 3 層記憶を統合管理し、検索、想起、保存、昇格、抽出を提供します。外部のベクトル DB や埋め込みモデルの違いは内部で吸収します。

## 主な構成

- `store.rs`: 3 層をまとめる `MemoryStore`
- `short_term.rs`: セッション単位のインメモリ記憶。`DashMap` と `Role::User / Assistant / Tool`
- `mid_term.rs`: 会話サマリーの保存、検索、保持期間クリーンアップ
- `long_term.rs`: 重要事実の保存、検索、削除。`search_by_guild` と `search_by_user` を提供
- `embedding.rs`: 埋め込み生成。OpenAI 互換と Mock フォールバック、リトライ処理
- `vector_db/mod.rs`: ベクトル DB の抽象インターフェース。`VectorDbClient`
- `vector_db/qdrant.rs`: Qdrant 実装。`session_scope_filter` とコサイン類似度
- `vector_db/inmemory.rs`: テスト向けのインメモリ実装

## 初期化ワークフロー: `MemoryStore::new` + `initialize`

1. `&AppConfig` から設定を読み込む
2. Qdrant クライアントを初期化する。URL と API key を使用
3. 埋め込みモデルを初期化する
   - 成功時: `OpenAICompatibleEmbedder` を使用し、Rig SDK と指数バックオフでリトライ
   - 失敗時: `MockEmbedder` にフォールバックし、`warn` を出す。FNV-1a ハッシュと LCG 疑似乱数を利用
4. `MidTermMemory` と `LongTermMemory` を構築する
5. `initialize().await` で両コレクションを `ensure_collection` する

### テスト用コンストラクタ

- `with_components(mid_term, long_term, embedder, short_term_max, mid_term_top_k, long_term_top_k)`: 既存コンポーネントを直接注入できる

## 短期記憶ワークフロー: `ShortTermMemory`

- `push_turn(session_key, user, assistant)`: 2 エントリを同一タイムスタンプで追加し、上限超過時は古いものから削除
- `get_messages(session_key)`: `Vec<ShortTermEntry>` を返す。各エントリには `role`、`content`、`timestamp` がある
- `get_count(session_key)`: 現在のエントリ数を返す
- `clear(session_key)`: セッション単位で削除する
- Role は `User`、`Assistant`、`Tool` の 3 種類

## 想起ワークフロー: `MemoryStore::recall`

1. `recall(session_key, query)` を呼ぶ
2. `embedder.embed(query)` でクエリを埋め込む
3. `tokio::join!` で中期記憶と長期記憶を並行検索
4. `RecalledMemory { mid_term, long_term }` を返す

### `should_summarize`

短期記憶のエントリ数が `max_entry` に達したかを判定します。

## 中期記憶ワークフロー

### `promote_to_mid_term`

1. 要約テキストを受け取り、埋め込みを生成
2. `mid_term` コレクションへ upsert
3. 短期記憶をクリア

### `promote_to_mid_term_with_messages`

会話メッセージから要約を生成して保存します。`clear_session` 時に使われます。

### 保存される payload

- `content`: 要約本文
- `guild_id`、`channel_id`、`kind`、`created_at`、`message_count`

### 検索

- `search(session_key, query, top_k)`: セッションスコープで検索し、`session_scope_filter` を適用
- `search_with_embedding(session_key, embedding, top_k)`: 埋め込み済みベクトルで検索

### 保持期間クリーンアップ

`MemoryStore::start_cleanup_job()` により、24 時間ごとに `delete_old_entries()` を実行し、`created_at < cutoff` の項目を削除します。

## 長期記憶ワークフロー

### `extract_long_term`

1. `facts: Vec<(String, Vec<String>)>` を受け取る
2. 各 fact の埋め込みを生成
3. `long_term` コレクションへ upsert

### 保存される payload

- `content`: 事実本文
- `guild_id`、`channel_id`、`kind`、`created_at`、`tags`、`user_id`

### 検索

- `search(session_key, query, top_k)`: セッションスコープ検索
- `search_by_guild(guild_id, query, top_k)`: ギルド単位検索
- `search_by_user(user_id, query, top_k)`: ユーザー単位検索
- `search_with_embedding(session_key, embedding, top_k)`: 埋め込み済み検索

### 削除

- `delete(id)`: ID 指定で削除
- `delete_by_channel(channel_id)`: チャンネル単位で削除

長期記憶は自動期限削除を行わず、明示削除のみです。

## ベクトル DB ワークフロー

`VectorDbClient` の主な操作は次のとおりです。

- `upsert(request)`: ベクトルと payload を保存
- `search(request)`: ベクトル検索とフィルタ検索を実行
- `delete(collection, id)`: ID を削除
- `delete_by_filter(collection, filter)`: フィルタで削除
- `ensure_collection(name, dim)`: コレクションを作成または確認

### Qdrant 実装

- Qdrant ネイティブの `Filter` / `Condition` に変換
- `session_scope_filter`: `guild_id`、`channel_id`、`kind` でフィルタリング
- リトライ戦略: 指数バックオフ、jitter、複数回再試行
- `SearchPointsBuilder` を使用

### InMemory 実装

- コサイン類似度と事前計算済みノルムでランキング
- `must` / `should` 条件をローカル評価
- `Default` を実装

## 埋め込みワークフロー

### `OpenAICompatibleEmbedder`

- Rig SDK の `openai::EmbeddingModel` をラップ
- 5 回のリトライを、指数バックオフ + jitter 付きで実行
- 全失敗時は `MockEmbedder` にフォールバック
- `f64` ベクトルを `Vec<f32>` に変換

### `MockEmbedder`

- FNV-1a ハッシュで安定した seed を生成
- LCG 疑似乱数でベクトルを生成
- テストや API 失敗時のフォールバック

## ヘルパー関数

### `search_result_to_entry`

`SearchResult` を `MemoryEntry` に変換します。

- `content`: payload から取得
- `score`: 検索スコア
- `created_at`: Unix タイムスタンプから `DateTime<Utc>` に変換
- `metadata`: payload 全体

## 連携ポイント

- `nekoui-agent`: `recall`、`promote_to_mid_term`、`extract_long_term`、`push_short_term`、`should_summarize`
- `nekoui-config`: 記憶設定と接続先
- `nekoui-domain`: `SessionKey` のスコープ情報
