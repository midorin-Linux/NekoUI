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