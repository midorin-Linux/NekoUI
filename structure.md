# nekoui ディレクトリ構造 v2（planning.md v4 準拠）

> planning.md v4 との突き合わせにより改訂。変更箇所は `# [NEW]` / `# [MOD]` で明示。

```
nekoui/
├── Cargo.toml
├── Cargo.lock
├── build.rs                  # バージョン情報・Git hash 埋め込み
│
├── data/                     # [NEW] ランタイムデータ（.gitignore 対象）
│   └── attachments/          # [NEW] アップロードファイル保存先 /data/attachments/{session_id}/
│       └── .gitkeep
│
└── src/
    ├── main.rs               # エントリポイント
    ├── lib.rs                # 統合エクスポート（テスト用）
    │
    ├── config.rs             # [MOD] 設定読み込み（SECRET_KEY・ALLOWED_ORIGINS・DB_PATH 含む）
    ├── error.rs              # [MOD] AppError・エラーコード（PAYLOAD_TOO_LARGE/413・SERVICE_UNAVAILABLE/503 追加済み）
    ├── state.rs              # [MOD] AppState（DBプール・サービス・設定 + admin_initialized フラグ保持）
    ├── constants.rs          # [MOD] レート制限値（Blueprint 準拠: 一般60/メッセージ20/upload10/auth10）
    │                         #       トークン有効期限（Access: 15分・Refresh: 30日）・ファイルサイズ上限（10MB）
    │
    ├── api/
    │   ├── mod.rs
    │   ├── router.rs         # 全エンドポイントのマウント
    │   ├── response.rs       # [MOD] 共通レスポンス { success, data, meta, error }（Blueprint 形式準拠）
    │   │                     #       meta: { request_id, total?, limit?, offset? }
    │   ├── pagination.rs     # [MOD] offset 方式（sessions/search/models）
    │   │                     #       + カーソル方式（messages: before/after/branch_id）
    │   ├── middleware/
    │   │   ├── mod.rs
    │   │   ├── auth.rs       # JWT 検証
    │   │   ├── rate_limit.rs # [MOD] Blueprint 値準拠 + X-RateLimit-Limit/Remaining/Reset ヘッダー付与
    │   │   │                 #       429 超過時は RATE_LIMITED エラーコードで返却
    │   │   ├── cors.rs       # [MOD] 開発: localhost:* / 本番: ALLOWED_ORIGINS 環境変数で制御
    │   │   ├── request_id.rs
    │   │   └── logging.rs
    │   └── routes/
    │       ├── mod.rs
    │       ├── auth.rs       # [MOD] register（初回のみ・管理者存在時403・成功時 token pair 返却）
    │       │                 #       login / logout / refresh / me / PATCH me
    │       ├── sessions.rs   # [MOD] CRUD + duplicate + export(?format=json|md|txt) + import(JSON/MD)
    │       │                 #       + DELETE /sessions（Body: {ids:[]}）+ merge（source_ids/target_id/strategy）
    │       ├── messages.rs   # [MOD] CRUD + regenerate（新バージョン追加方式・stream=true 対応）
    │       │                 #       + branch + versions + PATCH version + DELETE .../after
    │       │                 #       + POST /messages?stream=true（SSE 応答 = 方式A）
    │       ├── stream.rs     # [NEW] GET /sessions/{id}/stream（SSE 購読チャネル = 方式B）
    │       │                 #       セッションあたり同時1接続制限。SseManager 経由で配信
    │       ├── attachments.rs # multipart upload（10MB制限）/ 一覧 / メタデータ or ?download=true / 削除
    │       ├── providers.rs  # CRUD + test（latency_ms）+ sync
    │       ├── models.rs     # /providers/{id}/models/* + GET /models（横断一覧）+ capabilities
    │       ├── folders.rs    # [MOD] CRUD + DELETE ?cascade=true（セッションも削除）
    │       ├── tags.rs       # [MOD] GET /tags + POST /sessions/{id}/tags（Body: { tags: string[] }）
    │       │                 #       + DELETE /sessions/{id}/tags/{tag} + DELETE /tags/{tag}
    │       ├── presets.rs    # [MOD] CRUD + POST /presets/{id}/apply（override_model 対応）
    │       ├── settings.rs   # [MOD] GET/PATCH /settings（全カテゴリ: shortcuts/api_usage/mcp/tools/sandbox 含む）
    │       │                 #       + GET/PATCH /settings/{appearance|chat|notifications|privacy}
    │       │                 #       + POST /settings/reset（{ category?: string }）
    │       │                 #       + GET /settings/export（API キー除外）+ POST /settings/import
    │       ├── search.rs     # 全文検索専用（SearXNG は Tool 扱いのためここには含まない）
    │       │                 #       GET /search + /search/sessions + /search/messages
    │       └── system.rs     # [MOD] GET /health（public）+ GET /version（public）
    │                         #       + GET /status（auth）+ GET /metrics（admin 専用）
    │
    ├── services/
    │   ├── mod.rs
    │   ├── auth_service.rs   # [MOD] register（admin_initialized チェック）・login・refresh・logout・me
    │   ├── token_service.rs  # [MOD] JWT 発行/検証 + Refresh Token 発行/ローテーション（SQLite 永続）
    │   ├── session_service.rs # [MOD] CRUD + duplicate + export(json|md|txt) + import(JSON/MD)
    │   │                      #        + merge（source_ids/target_id/strategy: chronological|append）
    │   │                      #        + branch（新セッション生成・branch_from 記録）
    │   ├── message_service.rs # [MOD] CRUD + regenerate（message_versions へ新バージョン追加）
    │   │                      #        + after 削除 + context_builder 連携
    │   │                      #        + regenerate_following オプション（PATCH 時の後続再生成）
    │   ├── attachment_service.rs # upload（10MB/MIME バリデーション・/data/attachments/ 保存）
    │   │                         # + download + 削除（ストレージ + メタデータ）
    │   ├── provider_service.rs   # [MOD] CRUD + test（latency_ms）+ sync + APIKey 暗号化/復号/マスク
    │   ├── model_service.rs      # [MOD] CRUD + capabilities + sync（差分）+ 価格フィールド
    │   ├── folder_service.rs     # [MOD] CRUD + cascade 削除
    │   ├── tag_service.rs        # [MOD] CRUD + 使用頻度集計 + 複数タグ一括追加
    │   ├── preset_service.rs     # [MOD] CRUD + apply（override_model: false でセッションモデル維持）
    │   ├── settings_service.rs   # [MOD] 全カテゴリ管理 + reset（category 単位）+ export（APIKey 除外）
    │   ├── search_service.rs     # [MOD] 全文検索（InMemory: case-insensitive 文字列マッチ / SQLite: FTS5）
    │   └── system_service.rs     # health + version + status + metrics（admin）
    │
    ├── repositories/
    │   ├── mod.rs
    │   ├── traits/
    │   │   ├── mod.rs
    │   │   ├── chat.rs       # Session / Message / MessageVersion トレイト
    │   │   ├── user.rs
    │   │   ├── provider.rs   # Provider / Model / ModelCapabilities トレイト
    │   │   ├── attachment.rs # [NEW] Attachment トレイト（upload/list/get/delete）
    │   │   ├── settings.rs
    │   │   └── search.rs
    │   ├── memory/           # Phase 1〜12 で使用（RefreshToken を除く全データ）
    │   │   ├── mod.rs
    │   │   ├── chat_repository.rs
    │   │   ├── user_repository.rs
    │   │   ├── provider_repository.rs
    │   │   ├── attachment_repository.rs # [NEW] メタデータのみ InMemory 管理（実ファイルは /data/）
    │   │   ├── settings_repository.rs
    │   │   └── search_repository.rs
    │   └── sqlite/
    │       ├── mod.rs
    │       ├── refresh_token_repository.rs  # Phase 1 から使用（再起動耐性のため最初から SQLite）
    │       ├── chat_repository.rs           # Phase 13
    │       ├── user_repository.rs           # Phase 13
    │       ├── provider_repository.rs       # Phase 13
    │       ├── attachment_repository.rs     # [NEW] Phase 13
    │       ├── settings_repository.rs       # Phase 13
    │       ├── search_repository.rs         # Phase 13（FTS5 全文検索インデックス使用）
    │       └── migrations/
    │           ├── 001_refresh_tokens.sql   # [NEW] Phase 1 用（refresh_tokens テーブルのみ）
    │           └── 002_full_schema.sql      # [MOD] Phase 13 用（全テーブル + FTS5 インデックス）
    │                                        #        旧: 001_initial.sql
    │
    ├── models/               # ドメインモデル
    │   ├── mod.rs
    │   ├── user.rs
    │   ├── refresh_token.rs
    │   ├── session.rs        # [MOD] branch_from_session / branch_from_message フィールド含む
    │   ├── message.rs        # [MOD] active_version_id / status("interrupted") / input_tokens
    │   │                     #       / output_tokens / latency_ms フィールド含む
    │   ├── message_version.rs
    │   ├── attachment.rs
    │   ├── provider.rs
    │   ├── model.rs          # [MOD] input_price_per_1k / output_price_per_1k / description
    │   │                     #       / default_temperature / default_top_p フィールド含む
    │   ├── model_capabilities.rs # [MOD] json_mode / capabilities_override_json フィールド含む
    │   ├── folder.rs         # [MOD] color / icon フィールド含む
    │   ├── tag.rs
    │   ├── preset.rs         # [MOD] description フィールド含む
    │   └── settings.rs       # [MOD] shortcuts / api_usage / mcp / tools / sandbox カテゴリ含む
    │
    ├── dto/                  # リクエスト / レスポンス用 DTO
    │   ├── mod.rs
    │   ├── auth.rs
    │   ├── session.rs        # [MOD] ExportFormat(json|md|txt) / ImportBody / MergeBody(strategy)
    │   ├── message.rs        # [MOD] regenerate_following / カーソルページネーション(before/after/branch_id)
    │   ├── attachment.rs
    │   ├── provider.rs
    │   ├── model.rs          # [MOD] 価格フィールド
    │   ├── folder.rs         # [MOD] cascade / color / icon
    │   ├── tag.rs            # [MOD] { tags: string[] } 複数追加
    │   ├── preset.rs         # [MOD] override_model
    │   ├── settings.rs       # [MOD] 全カテゴリ（shortcuts/api_usage/mcp/tools/sandbox 含む）
    │   │                     #       SettingsResetBody { category?: String }
    │   ├── search.rs
    │   └── system.rs
    │
    ├── external/
    │   ├── mod.rs
    │   ├── provider/
    │   │   ├── mod.rs
    │   │   ├── adapter.rs            # Provider Adapter トレイト（非ストリーミング + ストリーミング両対応）
    │   │   ├── openai_compatible.rs  # OpenAI Compatible Adapter
    │   │   ├── openrouter.rs         # OpenRouter Adapter
    │   │   └── router.rs             # セッション紐づき Provider + Model 選択ロジック
    │   ├── mcp/
    │   │   ├── mod.rs
    │   │   ├── client_manager.rs  # stdio プロセス管理（起動確認・未起動時自動起動・クラッシュ検知）
    │   │   ├── process.rs
    │   │   └── protocol.rs
    │   └── searxng.rs             # [MOD] SearXNG HTTP クライアント（Tool 経由のみ使用・/search とは無関係）
    │
    ├── tools/
    │   ├── mod.rs
    │   ├── registry.rs    # ToolRegistry（内部 Tool + MCP Tool の統合管理）
    │   ├── dispatcher.rs  # [MOD] ラウンドトリップ実行（上限 10 回・無限ループ防止）
    │   │                  #       tool_use 検出 → Tool 実行 → 結果をコンテキストに追加 → 再帰
    │   ├── web_search.rs  # SearXNG Search Tool（ToolRegistry 経由でのみ呼び出される）
    │   ├── web_fetch.rs   # Web Fetch Tool
    │   ├── artifact.rs    # Artifact Tool
    │   └── sandbox.rs     # Sandbox Trait のみ（実装は後日）
    │
    ├── chat/
    │   ├── mod.rs
    │   ├── context_builder.rs  # [MOD] コンテキスト構築・ウィンドウ管理
    │   │                       #       トークン推定 → 上限比較 → 古いペアから削除（system/最新N保持）
    │   │                       #       context_truncated フラグをレスポンスに含める
    │   ├── streaming.rs        # [MOD] POST ?stream=true 用 SSE アダプタ（方式A）
    │   │                       #       切断検知 → 累積テキストで status:"interrupted" 保存
    │   ├── sse_manager.rs      # [NEW] GET /sessions/{id}/stream 用接続管理（方式B）
    │   │                       #       セッション毎の同時 1 接続制限・再接続・別タブ同期
    │   │                       #       AppState に保持する接続マップ（session_id → sender）
    │   └── sse_events.rs       # [MOD] SSE イベント定義（全種）
    │                           #       message.start / content.delta / content.done（Blueprint 追加）
    │                           #       / usage / message.stop / error
    │                           #       / tool.start / tool.result（Phase 11 拡張）
    │
    ├── utils/
    │   ├── mod.rs
    │   ├── crypto.rs    # AES-256-GCM（API Key 暗号化・復号）SECRET_KEY 環境変数依存
    │   ├── jwt.rs       # JWT 発行（Access: 15分）・検証
    │   ├── password.rs  # bcrypt / argon2
    │   ├── id.rs        # ID 生成
    │   └── mask.rs      # API Key マスク表示 "sk-••••••••1234"
    │
    └── constants.rs     # [MOD] レート制限値（Blueprint 準拠）・トークン有効期限・ファイル上限
                         #       RATE_LIMIT_GENERAL=60 / MESSAGE=20 / UPLOAD=10 / AUTH=10 (req/min)
                         #       ACCESS_TOKEN_EXPIRY=15min / REFRESH_TOKEN_EXPIRY=30days
                         #       ATTACHMENT_MAX_SIZE=10MB / TOOL_ROUNDTRIP_MAX=10

tests/
├── api_integration.rs     # 検証フロー 39 ステップの統合テスト
└── common/
    └── mod.rs             # [NEW] テストヘルパー（アプリ起動・DB 初期化・ヘッダー生成・SSE テスト）

.gitignore
```

---

## 変更サマリー

### 追加ファイル（[NEW]）

| パス | 理由 |
|---|---|
| `data/attachments/.gitkeep` | planning §20 の `/data/attachments/` 保存先。.gitignore 対象だが構造は明示 |
| `repositories/traits/attachment.rs` | Attachment トレイトが存在しなかった。memory/sqlite 両実装の共通契約 |
| `repositories/memory/attachment_repository.rs` | メタデータを InMemory で管理するPhase 1〜12 用実装 |
| `repositories/sqlite/attachment_repository.rs` | Phase 13 用 SQLite 実装 |
| `repositories/sqlite/migrations/001_refresh_tokens.sql` | **Refresh Token は Phase 1 から SQLite 必須**（サーバー再起動耐性）。旧 `001_initial.sql` から分離 |
| `repositories/sqlite/migrations/002_full_schema.sql` | Phase 13 の全テーブル + FTS5 インデックス。旧 `001_initial.sql` をリネーム・拡張 |
| `api/routes/stream.rs` | `GET /sessions/{id}/stream`（購読チャネル = 方式B）を messages.rs から独立させ責務を明確化 |
| `chat/sse_manager.rs` | 方式B 用の接続管理。セッション毎の同時1接続制限・AppState 上の接続マップ管理 |
| `tests/common/mod.rs` | 39 ステップ検証フローを支えるテストヘルパー |

### 修正コメント（[MOD]）の主要内訳

| ファイル | 修正内容（planning v4 決定事項との整合） |
|---|---|
| `state.rs` | `admin_initialized` フラグ保持を明示（register の初回制限に必要） |
| `constants.rs` | Blueprint 準拠のレート制限値・`TOOL_ROUNDTRIP_MAX=10` を明示 |
| `api/response.rs` | `meta.request_id` は全レスポンス共通、`total/limit/offset` はリスト系のみであることを明示 |
| `api/middleware/rate_limit.rs` | Blueprint 値と `X-RateLimit-*` ヘッダー 3 種を明示 |
| `api/routes/auth.rs` | register の初回制限・token pair 返却の仕様を明示 |
| `api/routes/messages.rs` | regenerate が「新バージョン追加方式」であること、`stream=true` で SSE 応答（方式A）であることを明示 |
| `api/routes/settings.rs` | `shortcuts/api_usage/mcp/tools/sandbox` カテゴリは `/settings` 一括経由、reset の `{ category? }` を明示 |
| `services/token_service.rs` | Refresh Token ローテーション（旧無効化→新発行→DB差替）フローを明示 |
| `services/session_service.rs` | merge の `strategy: chronological\|append`、export の 3 フォーマットを明示 |
| `services/settings_service.rs` | export 時の APIKey 除外を明示 |
| `models/message.rs` | `active_version_id / status / input_tokens / output_tokens / latency_ms` フィールド追加 |
| `models/model.rs` | 価格フィールド・`default_temperature/top_p/description` 追加 |
| `models/folder.rs` | `color / icon` フィールド追加 |
| `chat/sse_events.rs` | `content.done` イベント（Blueprint 追加）、`tool.start/tool.result`（Phase 11）を明示 |
| `chat/streaming.rs` | 方式A（POST ?stream=true）専用であること、切断検知→`interrupted` 保存を明示 |
| `tools/dispatcher.rs` | ラウンドトリップ上限 10 回を明示 |
| `external/searxng.rs` | `/search` エンドポイントとは無関係でありチャット内 Tool 経由のみ使用することを明示 |

### 削除・リネーム

| 変更前 | 変更後 | 理由 |
|---|---|---|
| `migrations/001_initial.sql` | `migrations/001_refresh_tokens.sql` + `migrations/002_full_schema.sql` | RefreshToken は Phase 1、残りは Phase 13 と明確に分離 |
