# `nekoai-domain` クレートのワークフロー

## 役割

`nekoai-domain` はクレート間で共有するドメイン型を定義します。セッション識別と呼び出し元コンテキストの型を提供します。

## 主な構成

- `agent/session.rs` (16行): `SessionKind` enum, `SessionKey` struct
- `agent/runtime.rs` (24行): `CallerContext` struct, `tokio::task_local!` 機構
- `agent/mod.rs` (2行): モジュール宣言
- `lib.rs` (1行): `pub mod agent;`

## 型定義

### `SessionKind`

Discord 上の会話コンテキストを 3 種類に正規化:
- `GuildChannel`: サーバー内通常チャンネル
- `Thread`: スレッド
- `DirectMessage`: DM

`Clone + Debug + Eq + PartialEq + Hash + Serialize` を導出。

### `SessionKey`

セッション識別子:
- `guild_id: Option<GuildId>`
- `channel_id: ChannelId`
- `thread_id: Option<ChannelId>`
- `kind: SessionKind`

`Eq + Hash` を持つため、セッションマップのキーとして利用可能。

### `CallerContext`

呼び出し元の識別情報:
- `user_id: Option<u64>`
- `guild_id: Option<u64>`

`Clone + Debug + Default` を導出。

## CallerContext 伝搬機構

`tokio::task_local!` を使用した暗黙的なコンテキスト伝搬:

```rust
tokio::task_local! {
    static CALLER_CONTEXT: RefCell<CallerContext>;
}
```

- `with_caller_context(context, future)`: future を指定された CallerContext でスコープ実行
- `current_caller_context()`: 現在のタスクから CallerContext を取得（未設定時はデフォルト）

これにより、明示的な引数なしで任意の非同期タスクから呼び出し元情報を参照可能。

## 利用ワークフロー

1. `nekoai-discord` が受信イベントから `SessionKey` を生成
2. `nekoai-agent` が `SessionKey` ごとにセッションを取得/更新、`CallerContext` で呼び出し元を追跡
3. `nekoai-memory` が `SessionKey` を使って検索フィルタを構築

## 設計上の位置づけ

- 外部 I/O 依存ロジックは持たない
- ビジネス境界の共通言語を提供する
- 上位層（CLI/Discord）と下位層（Agent/Memory）の接着点になる
