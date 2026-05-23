# `nekoui-domain` クレート ワークフロー

## 役割

`nekoui-domain` は、クレート間で共有するドメイン型を定義します。主にセッション識別と呼び出し元コンテキストを提供します。

## 主な構成

- `agent/session.rs`: `SessionKind` と `SessionKey`
- `agent/runtime.rs`: `CallerContext` と `tokio::task_local!` によるコンテキスト伝搬
- `agent/mod.rs`: モジュール宣言
- `lib.rs`: `pub mod agent;`

## 型定義

### `SessionKind`

Discord 上の会話コンテキストを 3 種類に正規化します。

- `GuildChannel`: サーバー上の通常チャンネル
- `Thread`: スレッド
- `DirectMessage`: DM

`Clone`、`Debug`、`Eq`、`PartialEq`、`Hash`、`Serialize` を実装します。

### `SessionKey`

セッションを一意に識別する型です。

- `guild_id: Option<GuildId>`
- `channel_id: ChannelId`
- `thread_id: Option<ChannelId>`
- `kind: SessionKind`

`Eq` と `Hash` を持つため、セッションマップのキーとして利用できます。

### `CallerContext`

呼び出し元の識別情報です。

- `user_id: Option<u64>`
- `guild_id: Option<u64>`

`Clone`、`Debug`、`Default` を実装します。

## `CallerContext` 伝搬機構

`tokio::task_local!` を使って、暗黙的に呼び出し元情報を引き回します。

```rust
tokio::task_local! {
    static CALLER_CONTEXT: RefCell<CallerContext>;
}
```

- `with_caller_context(context, future)`: 指定した `CallerContext` で future を実行
- `current_caller_context()`: 現在のタスクから `CallerContext` を取得。未設定時はデフォルト値を返す

これにより、明示的な引数なしで非同期タスクから呼び出し元情報を参照できます。

## 利用ワークフロー

1. `nekoui-discord` が受信イベントから `SessionKey` を生成
2. `nekoui-agent` が `SessionKey` ごとにセッションを管理し、`CallerContext` で呼び出し元を追跡
3. `nekoui-memory` が `SessionKey` を使って検索フィルタを構築

## 設計上の位置づけ

- 外部 I/O 依存のロジックは持たない
- ビジネスロジックの共通言語を提供する
- CLI / Discord と、Agent / Memory をつなぐ接着点になる
