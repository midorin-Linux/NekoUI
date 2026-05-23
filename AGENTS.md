# nekoui - Discord AI Agent

## プロジェクト概要

`nekoui` は Rust で開発されている AI エージェントです。Rig SDK を基盤とし、強力なメモリ管理（長期・中期・短期記憶）とツール実行機能を備えた、拡張性の高いチャットボットを提供します。

## クレート構造

このプロジェクトは Cargo workspace 形式のモノレポとして構成されています。各クレートの責務は以下の通りです。

### cli (`nekoui-cli`)
- エントリポイントとコマンドラインインターフェース
- `start` コマンドでエージェントを起動
- 設定ファイルの存在確認とセットアップウィザードの起動
- `--skip-setup` / `--token` / `--provider` / `--model` 引数の受け付け
- チャットクライアント、Discord 初期化と実行

### config (`nekoui-config`)
- 設定ファイルのロード・検証・JSON 形式
- 設定スキーマ定義（Discord、プロバイダ、メモリ設定、ツール権限など）
- `.config/config.json` からの設定読み込み
- Serialize/Deserialize の両方をサポート
- `SecretKey`: 出力時にマスク表示する安全なキー型
- メモリ設定項目
  - `short_term_max_entries` (default: 20): 短期記憶の最大エントリ数
  - `mid_term_top_k` (default: 3): 中期記憶検索時の上位 K 件
  - `long_term_top_k` (default: 5): 長期記憶検索時の上位 K 件
  - `mid_term_retention_days` (default: 30): 中期記憶の保持日数
  - `long_term_extraction_interval` (default: 10): 長期記憶抽出を実行するメッセージ間隔

### discord (`nekoui-discord`)
- Serenity による Discord イベント監視とコマンドルーティング
- `EventHandler` 実装、`ready` イベント
- スラッシュコマンド、`ask`、`clear`、`history` の登録と処理
- セッションキーの解決

### agent (`nekoui-agent`)
- Rig エージェントの推論ループとセッション管理
- `AgentRuntime`: 推論ループ、記憶想起、コンテキスト構築
- `SessionManager`: セッションごとの会話履歴管理
- `ContextManager`: コンテキスト構築（システムプロンプト、記憶注入）
- `ProviderAdapter`: OpenAI 互換プロバイダのラッパー

### memory (`nekoui-memory`)
- 3 層記憶システムの統合インターフェース
- `ShortTermMemory`: セッション単位のインメモリ `HashMap`
- `MidTermMemory`: 会話サマリーのベクトル保存、Qdrant
- `LongTermMemory`: 重要情報のベクトル保存、Qdrant
- `Embedding`: テキストをベクトルへ変換

### infra (`nekoui-infra`)
- ログ出力の初期化
- Tracing の設定

### domain (`nekoui-domain`)
- ビジネスロジック・共通データ型定義
- `SessionKey`、`SessionKind` の定義

### setup (`nekoui-setup`)
- 初回セットアップウィザード（dialoguer ベース、ステップ式）
- 設定ファイルの新規作成・既存ファイルとの差分マージ、既存値を優先
- CLI fallback モード（環境変数 `DISCORD_AGENT_TOKEN` / `--skip-setup` フラグ）
- `config_writer`: 設定ファイルの JSON 保存とマージロジック
- `cli_fallback`: コマンドライン引数から Config を構築

### tools (`nekoui-tools`)
- Rig `Tool` trait を実装したツールの定義
- `discord` モジュール: Discord API と連携するツール、`SendDiscordMessage` など
- 各ツールは `rig::tool::Tool` を実装し、エージェントから呼び出し可能
- 新しいツールを追加する場合は、`Tool` trait を実装し、`discord` モジュールまたは適切なモジュール内に配置する

## 開発環境

### 必要条件
- Rust の最新安定版
- Docker（Qdrant 使用時）
- just タスクランナー

### セットアップ
1. リポジトリをクローン
2. 設定ファイルを作成。初回起動時に自動的にセットアップウィザードが起動
3. Qdrant を起動（ベクトル検索を使用する場合）

## ビルドと実行

### ビルド
```bash
cargo build --bin nekoui-cli
```

### 実行
```bash
just neko start
```

### 開発用コマンド
```bash
just fmt  # フォーマット
cargo clippy -- -D warnings  # リンター
```

## コーディングスタイル

- Rust の標準的なコーディングスタイルに従う
- `cargo fmt` でフォーマット
- `cargo clippy` で静的解析
- トレーシングログを使用

## テスト方針

現在、テストは明示的に定義されていません。各クレートにユニットテストを追加することを推奨します。

## 既知の問題と制約

1. **設定ファイルのパス**: `.config/config.json` をルートディレクトリに配置
2. **Qdrant 接続**: デフォルトで `http://localhost:6334` に接続
3. **ツールシステム**: `SendDiscordMessage` ツールが実装済み。`AgentRuntime::add_tool()` で動的にツールを追加可能
4. ~~セットアップウィザード：TUI ベースのセットアップが計画されていたが、現在は CLI フォールバックのみ~~ → 実装済み：dialoguer ベースの 4 ステップウィザード + CLI fallback

## データフロー

1. Discord メッセージ受信 → セッションキー解決
2. `AgentRuntime.submit()` で推論開始
3. 記憶想起：中期・長期記憶から関連情報を検索
4. コンテキスト構築（短期記憶 + 想起した記憶）
5. Rig エージェントで推論実行（ツール呼び出しを含む）
6. 短期記憶に結果を保存
7. 会話を長期記憶抽出用バッファに蓄積
8. 蓄積メッセージ数が `long_term_extraction_interval` に達した場合、バッチ抽出タスクを非同期で実行（複数ターンから複数の事実を抽出）
9. 応答を Discord に送信

## 拡張性

- Web UI 拡張は `feature = "web-ui"` フラグで制御
- MCP サーバーは設定ファイルで定義可能
- カスタムツールは `ToolRegistry` に登録可能、`AgentRuntime::add_tool()` で動的追加
