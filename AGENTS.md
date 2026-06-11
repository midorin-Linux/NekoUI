# nekoui
NekoUIは強力な機能と、Rust採用による高速な処理と安全性を生かしたAIチャットアプリです。

## 開発する際の鉄則
- クレートを追加する際は`nekoui`配下の`Cargo.toml`に追加して、`nekoui`配下にあるクレート内のCargo.tomlに`{追加したクレート}.workspace = true`とするようにしてください。
- ローカルのクレート(`nekoui-｛文字｝`)は一番上に配置して通常のクレートは一行開けて文字順に追加するようにしてください。

## クレート構造
###

## リリースフロー

### バイナリ名
- `nekoui/app` クレートの package name は `nekoui` です。
- ビルド成果物は `nekoui.exe` (Windows) / `nekoui` (Linux/macOS) となります。
- ローカル実行: `cargo run -p nekoui -- start`

### GitHub Release
- `.github/workflows/release.yml` がリリースを担当します。
- **トリガー条件:**
  - `master` ブランチへのプッシュ → 自動で prerelease を作成（バージョン: `0.0.0-{sha}`）
  - `workflow_dispatch`（手動）→ 任意のバージョンで正式リリースを作成可能
- **リリース成果物の構成:**
  ```
  nekoui-{version}-{platform}/
  ├── nekoui(.exe)        ← Rust バックエンドバイナリ
  ├── gui/                 ← フロントエンドビルド成果物
  ├── docker-compose.yml
  ├── docker-settings.yml
  ├── LICENSE
  ├── README.md
  └── .env.example
  ```
- **対応プラットフォーム:** Linux (x86_64), Windows (x86_64), macOS (x86_64)
- **パッケージ形式:** Linux/macOS → `.tar.gz` / Windows → `.zip`

### CI（品質チェック）
- `.github/workflows/ci.yml` は PR / push 時に fmt, clippy, test, security, gui-lint を実行。
- リリースワークフローとは独立して動作し、品質ゲートの役割を担います。

## API サーバールート追加ルール

### Router-per-Module パターン
各 `routes/xxx.rs` は `pub fn router() -> Router<AppState>` を公開し、自身の全エンドポイントをその中で定義する。`server.rs` は各モジュールの `router()` を `.nest()` でまとめるだけ。

```rust
// routes/xxx.rs
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one).patch(patch).delete(delete))
}
```

```rust
// server.rs
let api_router = Router::new()
    .nest("/xxx", xxx::router())
    .with_state(app_state);
```

### ルートモジュール追加手順
1. `routes/xxx.rs` を作成（`pub fn router() -> Router<AppState>` を実装）
2. `routes/mod.rs` に `pub mod xxx;` を追加
3. `server.rs` の `build_routes()` に `.nest("/xxx", xxx::router())` を追加

### セッション依存ハンドラ
読み取り専用のセッションハンドラでは `ResolvedSession` extractor を使うと SessionKey 解決と Session 取得が自動化される。
書き込み操作（patch_session, delete_session）は内部で SessionManager 経由でのロック取得が必要なため、従来通り `State` + `Path` を使用する。

```rust
// 読み取り（ResolvedSession 推奨）
pub async fn get_something(resolved: ResolvedSession) -> impl IntoResponse {
    let guard = resolved.session.lock().await;
    ApiResponse::success(...)
}

// 書き込み（State + Path）
pub async fn write_something(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<...>,
) -> impl IntoResponse {
    let key = state.http_state.agent.session_manager().get_session_key(id)?;
    state.http_state.agent.session_manager().patch_session(key, ...).await?;
    ApiResponse::success(...)
}
```