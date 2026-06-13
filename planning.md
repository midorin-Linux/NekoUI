# バックエンド実装プランニング v4

> `blueprint.html` をAPI契約書として扱い、そこにないエンドポイントは原則追加しない。
> コード詳細よりワークフロー・データフロー・運用フロー優先。

---

## v3 からの変更点サマリー

> [!IMPORTANT]
> Blueprint との突き合わせ検証により、以下を修正しています。


| #  | 変更内容                                                                                | 理由                                                                                          |
| -- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| 1  | **`POST /messages` の `stream=true` を復活**                                            | Blueprint に「`stream=true` で SSE に切替可」と明記されている。v3 の「削除」は誤読            |
| 2  | **共通レスポンスを Blueprint 形式に統一**（`success` / `data` / `meta` / `error`）      | Blueprint の共通レスポンス形式に`success` があり、`meta` は `total/limit/offset` フラット構造 |
| 3  | **メッセージ一覧をカーソルページネーションに変更**（`limit, before, after, branch_id`） | Blueprint のクエリ仕様に準拠                                                                  |
| 4  | **Merge の Body を `{ source_ids[], target_id, strategy }` に修正**                     | Blueprint 準拠。新規セッション作成ではなく target への統合                                    |
| 5  | **Regenerate を「新バージョン追加」方式に修正**＋ `stream=true` 対応                    | Blueprint「同メッセージに新バージョンとして追加」                                             |
| 6  | **Rate Limit を Blueprint 値に統一**＋ `X-RateLimit-*` ヘッダー追加                     | Blueprint のレート制限定義に準拠                                                              |
| 7  | **SSE イベントに `content.done` を追加**                                              | Blueprint のイベント順序定義に含まれる                                                        |
| 8  | **Export に `format=json|md|txt`、Import に Markdown 対応を追加**                     | Blueprint のクエリ仕様に準拠                                                                  |
| 9  | **`GET /version` を public に変更**                                                     | Blueprint で public タグ                                                                      |
| 10 | **エラーコードに `PAYLOAD_TOO_LARGE`(413) / `SERVICE_UNAVAILABLE`(503) を追加**       | Blueprint のエラーレスポンス一覧に存在                                                        |
| 11 | **Settings カテゴリに `shortcuts` / `api_usage` を追加**                              | Blueprint の設定カテゴリ一覧に明記                                                            |
| 12 | **`DELETE /folders/{id}?cascade=true` を追加**                                          | Blueprint のクエリ仕様                                                                        |
| 13 | **`POST /presets/{id}/apply` に `override_model` を追加**                               | Blueprint の Body 仕様                                                                        |
| 14 | **`POST /settings/reset` に `{ category?: string }` を追加**                            | Blueprint：カテゴリ単位リセット対応                                                           |
| 15 | **Settings export は API キーを除外することを明記**                                     | Blueprint に明記                                                                              |
| 16 | **Models に価格フィールド（`input_price_per_1k` 等）を追加**                          | Blueprint の PATCH 可能フィールド一覧                                                         |
| 17 | **メッセージ編集後の「後続メッセージ再生成トリガー」オプションを追加**                | Blueprint の PATCH メッセージ説明                                                             |
| 18 | **`/auth/register` 成功時にトークンペアを返す（自動ログイン）**                         | Blueprint「access/refresh token を返す」                                                      |
| 19 | **タグ追加 Body を `{ tags: string[] }`（複数）に明記**                                 | Blueprint 準拠                                                                                |
| 20 | **Open Question 1（SSE の URL 長制限）を解決済みとしてクローズ**                        | `POST + stream=true` が正規ルートのため                                                       |

---

## 前提決定事項

### Q1: `/auth/register` は「初回のみ有効」か？

**→ YES。管理者アカウント未作成時のみ有効。**

```

管理者が存在しない → /auth/register 有効（成功時 access/refresh token を返す＝自動ログイン）
管理者が存在する   → /auth/register は 403 Forbidden
通常ログインは /auth/login のみ

```

判定ロジック: DB（またはインメモリフラグ）で「admin作成済み」フラグを保持。

> [!NOTE]
> Blueprint の `/auth/register` は `email, password, display_name` を受け取り、
> **access/refresh token を返す**仕様。登録成功＝ログイン済み状態になる。
> ログイン識別子は `email`。

---

### Q2: MCP設定は `/settings` 配下のカテゴリとして扱い、`/mcp/*` は作らない？

**→ YES。`/settings` 配下で管理する。**

```

settings.mcp.servers[]
  - name
  - command
  - args
  - env (secret はバックエンド保持)
  - enabled

```

理由: MCPはUIからの静的設定。実行時の動的API操作は不要。Blueprint契約の外に出ない。
カテゴリ別エンドポイントは Blueprint の4種（appearance/chat/notifications/privacy）のみ。
mcp / tools / sandbox / shortcuts / api_usage は `GET/PATCH /settings` 経由で操作する。

---

### Q3: SearXNG Web検索は `/search` ではなくチャット内toolとして扱う？

**→ YES。`/search` はセッション・メッセージの全文検索専用。SearXNGは内部Tool扱い。**

```

/search, /search/sessions, /search/messages → 全文検索
SearXNG → ToolRegistry 経由でチャット内のみ動作

```

---

### Q4（新規）: ストリーミングは2方式併存

**→ YES。Blueprint に両方が定義されている。**

```

方式A: POST /sessions/{id}/messages?stream=true
  → リクエスト Body でメッセージ送信、レスポンスが text/event-stream
  → 長文メッセージでも問題なし（正規ルート）

方式B: SSE GET /sessions/{id}/stream
  → 永続的なイベント購読チャネル
  → 生成中のデルタ配信・別タブ同期・再接続用

POST /sessions/{id}/messages/{msgId}/regenerate?stream=true も方式Aと同様に SSE 応答

```

---

### Q5（新規）: セッション削除は論理削除か物理削除か

**→ 物理削除を採用（初期実装）。**
Blueprint は「論理削除または物理削除」と両論併記のため実装側で決定。
ローカルツールでありゴミ箱UIも Blueprint にないため、シンプルな物理削除とする。
将来必要になれば `deleted_at` カラム追加で論理削除に移行可能。

---

## セキュリティ方針

### JWT戦略

```

Access Token  : 15分
Refresh Token : 30日

```

**⚠️ Refresh TokenはSQLiteに永続保存する（インメモリ不可）**

理由: 30日有効なRefresh Tokenをインメモリに置くと、サーバー再起動のたびに全ユーザーがログアウトになる。
Refresh Tokenのみ最初からSQLiteに保存し、他のデータはPhase 13でSQLite移行する。

トークンローテーション:

```

Refresh Token 使用
  ↓
新しいAccess Token発行
  ↓
新しいRefresh Token発行（古いものは無効化）
  ↓
DB上で新旧差し替え

```

---

### API Key保護

**⚠️ ハッシュ化ではなく暗号化（AES-256-GCM）**

理由: APIコール時に元の値を復元する必要があるためハッシュ化は不可。

```

保存時   : AES-256-GCM で暗号化してDBに格納
取り出し : 暗号化キーで復号してProvider Adapterに渡す
レスポンス: 常にマスク表示 → "sk-••••••••1234"
暗号化キー: 環境変数 (SECRET_KEY) で管理。DBには入れない

```

---

### Rate Limiting（Blueprint準拠）

```

一般 API（保護エンドポイント）: 60 req / min
メッセージ送信 (POST /messages, regenerate): 20 req / min
ファイルアップロード (POST /attachments, import系): 10 req / min
認証 (POST /auth/*): 10 req / min
SSE接続: セッションあたり同時1接続まで（独自追加・Blueprintと矛盾しない運用ルール)

```

レスポンスヘッダー（Blueprint準拠）:

```

X-RateLimit-Limit     : 上限値
X-RateLimit-Remaining : 残り回数
X-RateLimit-Reset     : リセット時刻（unix time）

```

超過時は `429 RATE_LIMITED` を返す。

---

### CORS

```

開発環境 : localhost:* 許可
本番環境 : 設定ファイル (ALLOWED_ORIGINS) で制御

```

---

## 共通レスポンス仕様（Blueprint準拠）

### 成功レスポンス

```json
{
  "success": true,
  "data": { },
  "meta": {
    "request_id": "uuid-v4",
    "total": 100,
    "limit": 20,
    "offset": 0
  },
  "error": null
}
```

- `meta.total / limit / offset` はリスト系レスポンスのみ含める（Blueprint のフラット構造を踏襲）
- `request_id` は全レスポンス共通（独自追加。Blueprint構造を壊さない拡張）

### エラーレスポンス

```json
{
  "success": false,
  "data": null,
  "meta": {
    "request_id": "uuid-v4"
  },
  "error": {
    "code": "ERROR_CODE",
    "message": "Human readable message"
  }
}
```

### エラーコード体系（Blueprint のHTTPステータス一覧を網羅）


| Code                  | HTTP Status | 説明                                         |
| --------------------- | ----------- | -------------------------------------------- |
| `VALIDATION_ERROR`    | 400         | リクエストパラメータ不正                     |
| `UNAUTHORIZED`        | 401         | 認証が必要、またはトークン無効               |
| `FORBIDDEN`           | 403         | 権限不足（register制限・admin専用など）      |
| `NOT_FOUND`           | 404         | リソースが見つからない                       |
| `CONFLICT`            | 409         | リソースの競合（重複作成など）               |
| `PAYLOAD_TOO_LARGE`   | 413         | アップロードサイズ超過                       |
| `RATE_LIMITED`        | 429         | レート制限超過                               |
| `INTERNAL_ERROR`      | 500         | サーバー内部エラー                           |
| `STREAM_ERROR`        | 500         | SSEストリーミング中のエラー                  |
| `PROVIDER_ERROR`      | 502         | AI Provider からのエラー                     |
| `SERVICE_UNAVAILABLE` | 503         | 依存サービス利用不可（DB・Provider全断など） |
| `MCP_TIMEOUT`         | 504         | MCP Server タイムアウト                      |

### ページネーション

#### offset方式（セッション・検索・モデル等）

リクエスト:

```

?limit=20&offset=0&sort=created_at&order=desc

```

レスポンス: `meta.total / limit / offset` で表現（上記成功レスポンス参照）。

対象: `GET /sessions`, `GET /search/*`, `GET /models`, `GET /providers` 等

#### カーソル方式（メッセージ一覧、Blueprint準拠）

```

GET /sessions/{id}/messages?limit=50&before={msgId}&after={msgId}&branch_id={branchId}

```

- `before`: 指定メッセージより前を取得（過去方向スクロール）
- `after` : 指定メッセージより後を取得（差分取得）
- `branch_id`: ブランチ指定（ブランチをセッション分割で実装する場合は省略可。後述）

---

## アーキテクチャレイヤー

```

┌─────────────────────────────────────────┐
│            Frontend (SPA)               │
└──────────────┬──────────────────────────┘
               │ HTTP / SSE
┌──────────────▼──────────────────────────┐
│         API Layer                        │
│  Router / Auth Middleware / Rate Limit   │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│        Service Layer                     │
│  AuthService / ChatService / etc.        │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│       Repository Layer                   │
│  InMemory ←→ SQLite (差し替え可能)        │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│       External Layer                     │
│  AI Provider / MCP stdio / SearXNG       │
└─────────────────────────────────────────┘

```
依存方向: 上から下のみ。下のレイヤーは上を参照しない。

---

## 全体ワークフロー

```

1. 初回起動
   ↓
2. /auth/register → admin作成（email, password, display_name）
   → access/refresh token を受け取る（自動ログイン）
   → 以降このエンドポイントは 403
   ↓
3. （2回目以降の利用時）/auth/login → Access Token + Refresh Token 発行
   ↓
4. Frontend が Bearer Token を保持
   ↓
5. 保護エンドポイント利用
   ↓
6. Provider + Model 設定（API Key 暗号化保存、モデル同期）
   ↓
7. Session 作成（オプション: Preset 適用、Folder 指定）
   ↓
8. Message 送信（非ストリーミング or stream=true で SSE）
   ↓
9. Tool / MCP 実行（必要な場合）
   ↓
10. Message 保存・表示

```

---

## データフロー詳細

### 1. 認証フロー

```

Frontend
  ↓ POST /auth/register { email, password, display_name }
    （初回のみ。管理者存在時は 403）
    → 成功時: access_token + refresh_token を返す（自動ログイン）

  または

  ↓ POST /auth/login { email, password }
Auth Service
  ↓ パスワードハッシュ検証 (bcrypt/argon2)
  ↓ 検証OK
Token Service
  ↓ Access Token 発行 (JWT, 15分)
  ↓ Refresh Token 発行 (opaque token, 30日)
  ↓ Refresh Token を SQLite に保存
Frontend
  ↓ Access Token: メモリ保持
  ↓ Refresh Token: HttpOnly Cookie または Secure Storage

```

---

### 2. トークンリフレッシュフロー

```

Access Token 期限切れ
  ↓
Frontend
  ↓ POST /auth/refresh { refresh_token }
Auth Service
  ↓ Refresh Token をDBで検索
  ↓ 有効期限チェック
  ↓ 新しい Access Token 発行
  ↓ 新しい Refresh Token 発行（ローテーション）
  ↓ 古い Refresh Token を無効化
  ↓ 新しい Refresh Token をDBに保存
Frontend
  ↓ 新しい Access Token を保持
  ↓ 元のリクエストをリトライ

```

リフレッシュ失敗時:

```

Refresh Token 無効 / 期限切れ
  ↓ 401 Unauthorized
  ↓ Frontend がログイン画面にリダイレクト

```

---

### 3. ログアウトフロー

```

Frontend
  ↓ POST /auth/logout
    Authorization: Bearer <access_token>
Auth Service
  ↓ リクエストから Refresh Token を取得（Body or Cookie）
  ↓ Refresh Token を revoked = true に設定
  ↓ 204 No Content
Frontend
  ↓ ローカルの Access Token / Refresh Token を破棄
  ↓ ログイン画面にリダイレクト

```

---

### 4. セッション作成フロー

```

Frontend
  ↓ POST /sessions { title?, model_id?, preset_id?, folder_id? }
Session Service
  ↓ Session ID 生成
  ↓ preset_id が指定されていれば Preset を適用
  ↓ folder_id が指定されていれば Folder に紐付け
  ↓ Repository に Session 保存
Frontend
  ↓ 作成された Session を受け取りサイドバーを更新

```

---

### 5. セッション一覧取得フロー

```

Frontend
  ↓ GET /sessions?q=...&folder_id=...&tag=...&model=...&limit=20&offset=0&sort=updated_at
Session Service
  ↓ フィルタ条件に基づいて Repository を検索
  ↓ ページネーション適用
  ↓ レスポンス: { success, data: Session[], meta: { total, limit, offset } }

```

`GET /sessions/{id}` の詳細レスポンスにはメタデータに加え統計情報（メッセージ数、合計トークン等）を含める（Blueprint準拠）。

---

### 6. セッション削除フロー

#### 単一削除

```

Frontend
  ↓ DELETE /sessions/{id}
Session Service
  ↓ 物理削除（Q5 の決定どおり）
  ↓ セッションに紐づくメッセージ・添付ファイルも削除
  ↓ 204 No Content

```

#### 一括削除（Blueprint準拠: Body `{ ids: string[] }`）

```

Frontend
  ↓ DELETE /sessions  Body: { ids: ["id1", "id2", "id3"] }
Session Service
  ↓ 指定された全セッションを削除
  ↓ 各セッションに紐づくメッセージ・添付ファイルも削除
  ↓ 204 No Content

```

> [!WARNING]
> `DELETE /sessions` は `{ ids: string[] }` による**選択的一括削除**。全削除は意図しない操作を防ぐため提供しない。

---

### 7. セッション複製フロー

```

Frontend
  ↓ POST /sessions/{id}/duplicate
Session Service
  ↓ 元セッションの全データを取得
  ↓ 新しい Session ID を生成
  ↓ メッセージ履歴をコピー（新しいメッセージIDで）
  ↓ タイトルに "(Copy)" を付与
  ↓ 新しいセッションを返す

```

---

### 8. セッション Export / Import フロー（Blueprint準拠: format対応）

#### Export

```

Frontend
  ↓ GET /sessions/{id}/export?format=json|md|txt（デフォルト: json）
Session Service
  ↓ format に応じて構築:
     json → セッション + メッセージ + メタデータの完全データ（再インポート可能）
     md   → Markdown形式の会話ログ（人間可読・再インポート可能）
     txt  → プレーンテキストの会話ログ（閲覧専用）
  ↓ Content-Disposition: attachment で返す

```

#### Import

```

Frontend
  ↓ POST /sessions/import  Body: エクスポートデータ（JSON または Markdown）
Session Service
  ↓ フォーマット判定（JSON / Markdown）
  ↓ パース・バリデーション
  ↓ 新しい Session ID で保存（元IDは使わない）
  ↓ メッセージも新規IDで保存
  ↓ 作成されたセッションを返す

```

---

### 9. メッセージ送信フロー（非ストリーミング）

```

Frontend
  ↓ POST /sessions/{id}/messages { role: "user", content: "...", attachment_ids?: [...] }
    （stream パラメータなし or stream=false）
Message Service
  ↓ User Message を Repository に保存
  ↓ attachment_ids があれば対応する Attachment をメッセージに紐付け
Context Builder
  ↓ セッションの過去メッセージを取得
  ↓ 添付ファイルをコンテキストに展開（テキスト→inline、画像→base64）
  ↓ コンテキストウィンドウチェック（後述）
Provider Router
  ↓ Session に紐づく Provider + Model を選択
Provider Adapter
  ↓ AI API を呼び出し（非ストリーミング）
  ↓ 応答受信
Message Service
  ↓ Assistant Message を Repository に保存
Frontend
  ↓ JSON レスポンスとして受け取り表示

```

エラー時:

```

Provider 呼び出し失敗
  ↓ Provider Adapter がエラーキャッチ
  ↓ User Message はそのまま保存（ロールバックしない）
  ↓ 502 PROVIDER_ERROR レスポンス
  ↓ Frontend がエラー表示・再試行ボタン表示

```

---

### 10. SSEストリーミングフロー（Blueprint準拠: 2方式）

#### 方式A: `POST /sessions/{id}/messages?stream=true`（メイン送信ルート）

```

Frontend
  ↓ POST /sessions/{id}/messages?stream=true
    Body: { role: "user", content: "...", attachment_ids?: [...] }
API Layer
  ↓ Content-Type: text/event-stream で応答開始
  ↓ User Message を保存
Context Builder
  ↓ 履歴整形・コンテキストチェック
Provider Router
  ↓ Provider + Model 選択
Streaming Adapter
  ↓ AI API ストリーミング呼び出し
  ↓ delta を受信するたびに SSE イベント送出

```

> [!IMPORTANT]
> 長文メッセージは Body で送れるため、v3 の Open Question 1（GET の URL 長制限）は解消。
> `POST ... regenerate?stream=true` も同じ仕組みで SSE 応答する。

#### 方式B: `SSE GET /sessions/{id}/stream`（購読チャネル）

```

Frontend
  ↓ EventSource("/sessions/{id}/stream") で接続
API Layer
  ↓ そのセッションで進行中の生成があればデルタを配信
  ↓ 再接続・別タブからの購読・生成中セッションへの復帰に使用

```

#### SSEイベント順序（Blueprint準拠 + Tool拡張）

```

event: message.start
data: { id, session_id, model, created_at }

event: content.delta
data: { index, delta: "テキスト断片" }
（繰り返し）

event: tool.start          ← 拡張イベント（Tool使用時のみ）
data: { tool_name, input }

event: tool.result         ← 拡張イベント（Tool使用時のみ）
data: { tool_name, output }

event: content.delta
data: { index, delta: "続きのテキスト" }

event: content.done        ← Blueprint準拠（本文確定）
data: { index }

event: usage
data: { input_tokens, output_tokens }

event: message.stop
data: { stop_reason: "end_turn" | "max_tokens" | "tool_use" }

--- または ---

event: error
data: { code, message }

```

```

Message Service
  ↓ delta を累積して完全なテキストを構築
  ↓ message.stop 受信後に Assistant Message を Repository に保存
SSE 接続クローズ

```

---

### 11. SSE切断・リカバリフロー

```

SSE 接続が途中で切断（ネットワーク障害など）
  ↓
Client-side
  ↓ EventSource の onerror をキャッチ
  ↓ Last-Event-ID を記録（実装する場合）
  ↓ 一定時間後に GET /sessions/{id}/stream で再接続試行

Server-side
  ↓ 接続切断を検知
  ↓ AI API への接続を中断（初期実装）
  ↓ それまでの累積テキストで Message を保存（status: "interrupted"）

Frontend 再接続時:
  ↓ interrupted メッセージを表示
  ↓ 「続きを生成」ボタンで regenerate 呼び出し

```

---

### 12. コンテキストウィンドウ管理フロー

```

Context Builder
  ↓ セッションの全メッセージ取得
  ↓ トークン数を推定（文字数ベース簡易計算 or tiktoken相当）
  ↓ Model の context_window 上限と比較

超過しない場合:
  ↓ 全メッセージをそのまま使用

超過する場合（優先度順に削除）:
  1. 古い assistant/user メッセージペアから削除
  2. system prompt は保持
  3. 最新 N ターンは保持（設定可能、デフォルト: 10）
  4. 削除後もまだ超過 → エラー（メッセージが長すぎる）

Frontend への通知:
  ↓ レスポンスに context_truncated: true フラグを含める
  ↓ UI で「古いメッセージが省略されました」を表示

```

---

### 13. MCPツール実行フロー（ラウンドトリップ）

```

Chat Service
  ↓ LLM が tool_use を応答
  ↓ stop_reason: "tool_use" を検出

Tool Dispatcher
  ↓ tool 名を ToolRegistry で検索
  ↓ 内部 Tool か MCP Tool かを判定

MCP Tool の場合:
  ↓ McpClientManager に対象 server への接続要求
  ↓ stdio MCP Server が起動済みか確認
  ↓ 未起動なら起動（プロセス管理）
  ↓ MCP プロトコルで tool 呼び出し
  ↓ 結果受信

内部 Tool の場合（SearXNG / Web Fetch）:
  ↓ ToolRegistry から該当 Tool を取得
  ↓ Tool を実行
  ↓ 結果受信

Tool 結果をコンテキストに追加:
  ↓ role: "tool", content: [result]
  ↓ LLM に続きを要求（再帰的にtool_useが来る可能性あり）
  ↓ LLM が最終応答を生成

SSE での表示:
  ↓ event: tool.start  { tool_name, input }
  ↓ event: tool.result { tool_name, output }
  ↓ event: content.delta（続きのテキスト）
  ↓ event: content.done → usage → message.stop

Tool ラウンドトリップ上限:
  ↓ 最大 10回（無限ループ防止）
  ↓ 超過時はエラーを返す

エラー時:
  ↓ MCP Server が応答しない → タイムアウト（デフォルト: 30秒、MCP_TIMEOUT）
  ↓ Tool 実行エラー → エラー内容を LLM に返してフォールバック応答
  ↓ MCP Server クラッシュ → 接続を切断しエラーをログ、次回リクエスト時に再起動

```

---

### 14. Providerテスト・同期フロー

#### Provider テスト

```

Frontend
  ↓ POST /providers/{id}/test
Provider Service
  ↓ DBから Provider 情報を取得
  ↓ API Key を復号
  ↓ Provider Adapter で簡易リクエスト送信（例: モデル一覧取得）
  ↓ 成功: { status: "ok", latency_ms: 123 }
  ↓ 失敗: { status: "error", reason: "..." }

```

#### Provider 同期（`POST /providers/{id}/sync`）

```

Frontend
  ↓ POST /providers/{id}/sync
Provider Service
  ↓ Provider の API からモデル一覧を取得
  ↓ 既存モデルとの差分を計算
  ↓ 新規モデル → DBに追加（enabled: false デフォルト）
  ↓ 削除されたモデル → enabled: false にする（物理削除しない）
  ↓ 既存モデル → 変更がある場合のみ更新
  ↓ 同期結果サマリーを返す
     { added: 3, updated: 1, disabled: 0 }

```

#### モデル単位の同期（`POST /providers/{id}/models/sync`）

```

Blueprint では両方とも「モデル一覧の差分同期」と説明されている。
→ 両方とも同じ「モデル一覧同期」を実行し、内部では同一 Service メソッドを呼ぶ。
  （/providers/{id}/sync はエイリアス扱い）

```

---

### 15. メッセージバージョン管理フロー

#### メッセージ編集（Blueprint準拠: 後続再生成トリガー対応）

```

Frontend
  ↓ PATCH /sessions/{id}/messages/{msgId}
    Body: { content: "新しい内容", regenerate_following?: boolean }
Message Service
  ↓ 既存メッセージの content を message_versions テーブルにコピー
  ↓ 新しい content で既存メッセージを更新
  ↓ version カウントをインクリメント
  ↓ regenerate_following: true の場合:
     - msgId 以降の assistant メッセージを再生成対象としてマーク
     - 直後の assistant メッセージの regenerate を内部的に実行
  ↓ 更新後のメッセージを返す

```

#### バージョン一覧取得

```

Frontend
  ↓ GET /sessions/{id}/messages/{msgId}/versions
Message Service
  ↓ message_versions から過去の content 一覧を取得
  ↓ 作成日時順（降順）で返す

```

#### バージョン切り替え

```

Frontend
  ↓ PATCH /sessions/{id}/messages/{msgId}/version { version_id: "..." }
Message Service
  ↓ active_version_id を指定された version_id に変更
  ↓ 更新後のメッセージを返す

```

---

### 16. Regenerate フロー（Blueprint準拠: 新バージョン追加方式）

```

Frontend
  ↓ POST /sessions/{id}/messages/{msgId}/regenerate（?stream=true 対応）
Message Service
  ↓ msgId が role: "assistant" であることを確認
  ↓ msgId の直前の User Message を特定
Context Builder
  ↓ 直前までのコンテキストを構築
Provider Adapter
  ↓ 再生成（stream=true なら SSE、なければ JSON）
Message Service
  ↓ 新しい応答を message_versions に「新バージョンとして追加」
  ↓ active_version_id を新バージョンに切り替え
  ↓ （上書きではない。過去バージョンは versions API で参照・切替可能）
  ↓ 更新後のメッセージを返す

```

> [!NOTE]
> Blueprint:「AI 応答を再生成。**同メッセージに新バージョンとして追加**」。
> 編集（PATCH）とregenerateの両方が message_versions に履歴を積む統一モデルとする。

---

### 17. Branch フロー

```

Frontend
  ↓ POST /sessions/{id}/messages/{msgId}/branch
Session Service
  ↓ 現在のセッションを基に新しいセッションを作成
  ↓ msgId 以前のメッセージを新セッションにコピー
  ↓ branch_from: { session_id, message_id } を新セッションに記録
  ↓ 新しいセッション ID を返す
Frontend
  ↓ 新セッションを開く

```

> [!NOTE]
> Blueprint のメッセージ一覧クエリに `branch_id` があるが、本実装ではブランチを
> 「独立した新セッション」として表現するため `branch_id` は当面未使用（受け取っても無視）。
> セッション内マルチブランチが必要になった場合に拡張する。

---

### 18. Merge フロー（Blueprint準拠に修正）

```

Frontend
  ↓ POST /sessions/merge
    Body: { source_ids: ["id1", "id2"], target_id: "id3", strategy: "chronological" }
Session Service
  ↓ target_id のセッションを取得
  ↓ 各ソースセッションのメッセージを取得
  ↓ strategy に従ってマージ:
     - "chronological": created_at 順に時系列マージ（デフォルト）
     - "append"       : source_ids の順に末尾へ連結
  ↓ マージ結果を target セッションに追加保存
  ↓ マージ元セッションは保持（削除しない）
  ↓ 更新後の target セッションを返す

```

> [!WARNING]
> v3 の「新規セッションを作成」方式は Blueprint と不一致。
> Blueprint の Body は `{ source_ids[], target_id, strategy }` であり、**既存の target に統合**する。

---

### 19. メッセージ以降削除フロー

```

Frontend
  ↓ DELETE /sessions/{id}/messages/{msgId}/after
Message Service
  ↓ msgId の created_at 以降のメッセージを全削除
  ↓ msgId 自体は残す
  ↓ 204 No Content

```

用途: 会話を特定のポイントまで巻き戻してやり直す場合。

---

### 20. 添付ファイルフロー

#### アップロード

```

Frontend
  ↓ POST /sessions/{id}/attachments
    Content-Type: multipart/form-data
    file: <binary>
Attachment Service
  ↓ ファイルサイズチェック（上限: 10MB、超過時 413 PAYLOAD_TOO_LARGE）
  ↓ MIMEタイプバリデーション（画像・PDF・テキスト）
  ↓ ローカル保存: /data/attachments/{session_id}/{attachment_id}_{filename}
  ↓ メタデータを Repository に保存
  ↓ Attachment オブジェクトを返す（id, filename, mime_type, size, url）

```

#### メッセージへの紐付け

```

メッセージ送信時:
  POST /sessions/{id}/messages
  Body: { content: "...", attachment_ids: ["att_1", "att_2"] }

Context Builder:
  ↓ attachment_ids から Attachment を取得
  ↓ テキストファイル → content として展開
  ↓ 画像ファイル → base64 エンコードして vision 対応で送信
  ↓ PDF → テキスト抽出して展開（抽出不可ならメタ情報のみ）
  ↓ その他のファイル → ファイル名とメタ情報のみコンテキストに追加

```

#### 一覧・取得・削除

```

GET    /sessions/{id}/attachments               → セッション内の添付一覧
GET    /sessions/{id}/attachments/{attachId}     → メタデータ取得 or バイナリダウンロード
       （?download=true でバイナリ、デフォルトはメタデータJSON）
DELETE /sessions/{id}/attachments/{attachId}     → ファイル削除（ストレージ + メタデータ）

```

---

### 21. プリセット適用フロー（Blueprint準拠: override_model対応）

```

Frontend
  ↓ POST /presets/{id}/apply { session_id: "...", override_model?: boolean }
Preset Service
  ↓ Preset を取得
  ↓ Session に以下を上書き:
     - system_prompt
     - model_id（override_model: false の場合はセッションの現行モデルを維持）
     - temperature, top_p
     - tool enabled flags
     - MCP enabled servers
  ↓ 更新後の Session を返す

```

セッション作成時に preset_id を指定した場合は自動適用。

---

### 22. Settings フロー

#### カテゴリ別取得・更新（カテゴリ別エンドポイントは Blueprint の4種のみ）

```

GET  /settings                → 全設定を取得（全カテゴリをフラットに返す）
PATCH /settings               → 複数カテゴリを一括更新
                                （shortcuts / api_usage / tools / mcp / sandbox はここ経由）

GET  /settings/appearance     → テーマ・フォントサイズ・フォントファミリー・コードハイライト・言語
PATCH /settings/appearance

GET  /settings/chat           → デフォルトモデル・温度・最大トークン・自動タイトル生成 等
PATCH /settings/chat

GET  /settings/notifications  → プッシュ・メール・サウンド
PATCH /settings/notifications

GET  /settings/privacy        → データ保存期間・使用状況送信・セッション暗号化
PATCH /settings/privacy

```

#### Settings の構造（Blueprint カテゴリ + 独自カテゴリ）

```json
{
  "appearance": {
    "theme": "dark",
    "font_size": 14,
    "font_family": "system",
    "code_highlight": "github-dark",
    "language": "ja"
  },
  "chat": {
    "default_model_id": null,
    "default_temperature": 0.7,
    "max_tokens": 4096,
    "stream_enabled": true,
    "auto_title": true,
    "context_max_turns": 10,
    "context_strategy": "sliding_window"
  },
  "notifications": {
    "push_enabled": false,
    "email_enabled": false,
    "sound_enabled": true
  },
  "privacy": {
    "save_history": true,
    "retention_days": 0,
    "analytics_enabled": false
  },
  "shortcuts": {
    "send_message": "Enter",
    "new_session": "Ctrl+N"
  },
  "api_usage": {
    "monthly_token_limit": 0,
    "alert_threshold_percent": 80
  },
  "tools": {
    "web_search": { "enabled": false },
    "web_fetch": { "enabled": false }
  },
  "mcp": {
    "servers": []
  },
  "sandbox": {
    "provider": "none",
    "docker": { "image": "" }
  }
}
```

#### リセット / Export / Import（Blueprint準拠）

```

POST /settings/reset   Body: { category?: string }
  → category 指定: そのカテゴリのみデフォルトに戻す
  → 省略: 全カテゴリをリセット

GET  /settings/export
  → 全設定を JSON でエクスポート
  → ⚠️ API キー・MCP env の secret は除外する（Blueprint 明記）

POST /settings/import
  → JSON から設定をインポートして上書き
  → API キー類は含まれないため Provider 設定は別途必要

```

---

### 23. 検索フロー（Blueprint準拠: フィルタ追加）

```

Frontend
  ↓ GET /search?q=keyword&type=all&from=2026-01-01&to=2026-06-01&model_id=...&limit=20&offset=0
Search Service
  ↓ sessions と messages を横断検索
  ↓ from/to で日付範囲フィルタ、model_id でモデルフィルタ
  ↓ マッチしたセッション + メッセージ（スニペット付き）を返す

  ↓ GET /search/sessions?q=keyword
  ↓ セッションのタイトル・システムプロンプトを検索

  ↓ GET /search/messages?q=keyword
  ↓ メッセージ本文のみ検索。前後スニペット付きで返す

```

検索エンジン:

- インメモリ期間: 単純な文字列マッチング（case-insensitive）
- SQLite移行後: FTS5 全文検索

---

## APIエンドポイント一覧（Blueprint準拠）

### Auth（6 endpoints）


| Method | Endpoint         | 説明                                                           |
| ------ | ---------------- | -------------------------------------------------------------- |
| POST   | `/auth/register` | 初回のみ有効。email, password, display_name → token pair 返却 |
| POST   | `/auth/login`    | ログイン                                                       |
| POST   | `/auth/logout`   | ログアウト（Refresh Token無効化）                              |
| POST   | `/auth/refresh`  | トークンリフレッシュ                                           |
| GET    | `/auth/me`       | ユーザー情報取得                                               |
| PATCH  | `/auth/me`       | プロフィール更新（表示名・パスワード・アバター）               |

### Sessions（9 endpoints）


| Method | Endpoint                   | 説明                                                             |
| ------ | -------------------------- | ---------------------------------------------------------------- |
| GET    | `/sessions`                | 一覧（`q, folder_id, tag, model, limit, offset, sort`）          |
| POST   | `/sessions`                | 新規作成                                                         |
| GET    | `/sessions/{id}`           | 詳細取得（統計情報含む）                                         |
| PATCH  | `/sessions/{id}`           | 更新（タイトル・フォルダ・モデル・システムプロンプト・ピン留め） |
| DELETE | `/sessions/{id}`           | 単一削除（物理削除）                                             |
| DELETE | `/sessions`                | 一括削除（Body:`{ ids: string[] }`）                             |
| POST   | `/sessions/{id}/duplicate` | 複製                                                             |
| GET    | `/sessions/{id}/export`    | エクスポート（`format=json|md|txt`）                             |
| POST   | `/sessions/import`         | インポート（JSON/Markdown対応）                                  |

### Messages + SSE（12 endpoints）


| Method | Endpoint                                     | 説明                                                   |
| ------ | -------------------------------------------- | ------------------------------------------------------ |
| GET    | `/sessions/{id}/messages`                    | 一覧（`limit, before, after, branch_id` カーソル方式） |
| POST   | `/sessions/{id}/messages`                    | 送信。`stream=true` で SSE 切替                        |
| GET    | `/sessions/{id}/messages/{msgId}`            | 単一取得（トークン数・所要時間含む）                   |
| PATCH  | `/sessions/{id}/messages/{msgId}`            | 編集（後続再生成トリガー可）                           |
| DELETE | `/sessions/{id}/messages/{msgId}`            | 削除                                                   |
| DELETE | `/sessions/{id}/messages/{msgId}/after`      | 以降削除（巻き戻し）                                   |
| POST   | `/sessions/{id}/messages/{msgId}/regenerate` | 再生成（新バージョン追加、`stream=true` 対応）         |
| GET    | `/sessions/{id}/messages/{msgId}/versions`   | バージョン一覧                                         |
| PATCH  | `/sessions/{id}/messages/{msgId}/version`    | バージョン切替（`{ version_id }`）                     |
| POST   | `/sessions/{id}/messages/{msgId}/branch`     | ブランチ（フォーク）                                   |
| POST   | `/sessions/merge`                            | マージ（`{ source_ids[], target_id, strategy }`）      |
| SSE    | `/sessions/{id}/stream`                      | ストリーム購読チャネル                                 |

### Attachments（4 endpoints）


| Method | Endpoint                                | 説明                                |
| ------ | --------------------------------------- | ----------------------------------- |
| GET    | `/sessions/{id}/attachments`            | 一覧                                |
| POST   | `/sessions/{id}/attachments`            | アップロード（multipart、上限10MB） |
| GET    | `/sessions/{id}/attachments/{attachId}` | メタデータ取得 or ダウンロード      |
| DELETE | `/sessions/{id}/attachments/{attachId}` | 削除                                |

### Providers（7 endpoints）


| Method | Endpoint               | 説明                                    |
| ------ | ---------------------- | --------------------------------------- |
| GET    | `/providers`           | 一覧（API キーはマスク表示）            |
| POST   | `/providers`           | 追加（`type, name, api_key, base_url`） |
| GET    | `/providers/{id}`      | 詳細（有効モデル一覧・ステータス含む）  |
| PATCH  | `/providers/{id}`      | 更新                                    |
| DELETE | `/providers/{id}`      | 削除（関連モデルも削除）                |
| POST   | `/providers/{id}/test` | 接続テスト（レイテンシ計測込み）        |
| POST   | `/providers/{id}/sync` | モデル一覧同期                          |

### Models（9 endpoints）


| Method | Endpoint                                        | 説明                                                |
| ------ | ----------------------------------------------- | --------------------------------------------------- |
| GET    | `/models`                                       | 横断一覧（`provider_id, enabled, supports_vision`） |
| GET    | `/providers/{id}/models`                        | Provider別一覧                                      |
| POST   | `/providers/{id}/models`                        | 手動追加（カスタムモデル等）                        |
| GET    | `/providers/{id}/models/{modelId}`              | 詳細（コンテキスト・料金・機能）                    |
| PATCH  | `/providers/{id}/models/{modelId}`              | 更新（表示名・価格・温度・有効フラグ等）            |
| DELETE | `/providers/{id}/models/{modelId}`              | 削除（利用中セッションへの警告付き）                |
| GET    | `/providers/{id}/models/{modelId}/capabilities` | Capabilities取得                                    |
| PATCH  | `/providers/{id}/models/{modelId}/capabilities` | Capabilitiesオーバーライド                          |
| POST   | `/providers/{id}/models/sync`                   | モデル差分同期                                      |

### Folders / Tags（8 endpoints）


| Method | Endpoint                    | 説明                                           |
| ------ | --------------------------- | ---------------------------------------------- |
| GET    | `/folders`                  | 一覧（ツリー構造）                             |
| POST   | `/folders`                  | 作成（`name, parent_id, color, icon`）         |
| PATCH  | `/folders/{id}`             | 更新（名前・移動・色・アイコン）               |
| DELETE | `/folders/{id}`             | 削除（`?cascade=true` で中のセッションも削除） |
| GET    | `/tags`                     | 一覧（使用頻度・件数付き）                     |
| POST   | `/sessions/{id}/tags`       | タグ追加（Body:`{ tags: string[] }`）          |
| DELETE | `/sessions/{id}/tags/{tag}` | セッションからタグ削除                         |
| DELETE | `/tags/{tag}`               | タグ自体を削除（全セッションから除去）         |

### Presets（6 endpoints）


| Method | Endpoint              | 説明                                     |
| ------ | --------------------- | ---------------------------------------- |
| GET    | `/presets`            | 一覧                                     |
| POST   | `/presets`            | 作成                                     |
| GET    | `/presets/{id}`       | 詳細                                     |
| PATCH  | `/presets/{id}`       | 更新                                     |
| DELETE | `/presets/{id}`       | 削除                                     |
| POST   | `/presets/{id}/apply` | 適用（`{ session_id, override_model }`） |

### Settings（13 endpoints）


| Method | Endpoint                  | 説明                                                           |
| ------ | ------------------------- | -------------------------------------------------------------- |
| GET    | `/settings`               | 全設定取得                                                     |
| PATCH  | `/settings`               | 一括更新（shortcuts / api_usage / tools / mcp / sandbox 含む） |
| GET    | `/settings/appearance`    | 外観設定                                                       |
| PATCH  | `/settings/appearance`    | 外観設定更新                                                   |
| GET    | `/settings/chat`          | チャット設定                                                   |
| PATCH  | `/settings/chat`          | チャット設定更新                                               |
| GET    | `/settings/notifications` | 通知設定                                                       |
| PATCH  | `/settings/notifications` | 通知設定更新                                                   |
| GET    | `/settings/privacy`       | プライバシー設定                                               |
| PATCH  | `/settings/privacy`       | プライバシー設定更新                                           |
| POST   | `/settings/reset`         | リセット（`{ category?: string }`）                            |
| GET    | `/settings/export`        | エクスポート（API キー除外）                                   |
| POST   | `/settings/import`        | インポート                                                     |

> [!NOTE]
> Blueprint のサイドバー集計は 12 だが、テーブルには 13 行存在する（Blueprint側の集計ずれ）。
> 実装は上記テーブル（13）に従う。

### Search（3 endpoints）


| Method | Endpoint           | 説明                                             |
| ------ | ------------------ | ------------------------------------------------ |
| GET    | `/search`          | 横断検索（`q, type, from, to, model_id, limit`） |
| GET    | `/search/sessions` | セッション検索（タイトル・システムプロンプト）   |
| GET    | `/search/messages` | メッセージ検索（スニペット付き）                 |

### System（4 endpoints）


| Method | Endpoint   | 認証   | 説明                                           |
| ------ | ---------- | ------ | ---------------------------------------------- |
| GET    | `/health`  | public | ヘルスチェック（DB・外部依存ステータス）       |
| GET    | `/version` | public | バージョン・ビルド情報・対応機能リスト         |
| GET    | `/status`  | auth   | Provider稼働状況・レスポンスタイム             |
| GET    | `/metrics` | admin  | トークン数・コスト・リクエスト数・エラーレート |

---

## SQLite移行方針

### Refresh TokenのみPhase 1からSQLite

```

RefreshTokenRepository
  └── SQLite（最初から）

```

### その他のデータはPhase 13で移行

```

Repository Interface（共通）
  ├── InMemoryChatRepository（Phase 1〜12で使用）
  └── SqliteChatRepository（Phase 13で差し替え）

```

移行時にAPIとビジネスロジックは変更しない。Repository の実装だけを差し替える。

### SQLite テーブル設計（移行時の参考）

```sql
users             : id, email, display_name, password_hash, avatar_url, created_at
refresh_tokens    : id, user_id, token_hash, expires_at, revoked, created_at
sessions          : id, user_id, title, model_id, system_prompt, folder_id, pinned,
                    branch_from_session, branch_from_message, created_at, updated_at
messages          : id, session_id, role, content, active_version_id, status,
                    input_tokens, output_tokens, latency_ms, created_at
message_versions  : id, message_id, content, created_at
providers         : id, name, type, api_key_encrypted, base_url, enabled, created_at
models            : id, provider_id, model_id, display_name, description,
                    default_temperature, default_top_p,
                    input_price_per_1k, output_price_per_1k,
                    enabled, created_at
model_capabilities: id, model_id, context_window, max_output_tokens,
                    vision, function_calling, json_mode, streaming,
                    capabilities_override_json
settings          : key, value, category, updated_at
presets           : id, name, description, config_json, created_at, updated_at
folders           : id, name, parent_id, color, icon, sort_order, created_at
tags              : session_id, tag
attachments       : id, session_id, message_id, filename, mime_type, size, path, created_at
```

> [!NOTE]
> v3 との差分:
>
> - `messages` に `active_version_id`（バージョン切替用）、`input_tokens / output_tokens / latency_ms`
>   （Blueprint「メタデータ・トークン数・所要時間含む」）を追加
> - `models` に `description`, `default_temperature`, `default_top_p`,
>   `input_price_per_1k`, `output_price_per_1k` を追加（Blueprint の PATCH 可能フィールド）
> - `model_capabilities` に `json_mode`, `capabilities_override_json` を追加
> - `folders` に `color`, `icon` を追加（Blueprint の作成パラメータ）
> - `presets` に `description` を追加

---

## 実装フェーズ

### フェーズ間の依存関係

```

graph LR
    P1[Phase 1: API基盤] --> P2[Phase 2: Auth]
    P2 --> P3[Phase 3: Providers/Models]
    P2 --> P4[Phase 4: Sessions]
    P3 --> P5[Phase 5: Messages]
    P4 --> P5
    P3 --> P6[Phase 6: SSE]
    P5 --> P6
    P4 --> P7[Phase 7: Attachments]
    P2 --> P8[Phase 8: Settings]
    P4 --> P9[Phase 9: Folders/Tags/Presets]
    P5 --> P10[Phase 10: Search]
    P6 --> P11[Phase 11: Tools/MCP]
    P1 --> P12[Phase 12: System]
    P12 --> P13[Phase 13: SQLite移行]

```

### Phase 1: API基盤

- 共通レスポンス形式の確定（`success` / `data` / `meta` / `error`、Blueprint準拠）
- エラーコード体系の実装（413 / 503 含む）
- ページネーションヘルパー（offset方式 + カーソル方式の両方）
- Middleware: 認証チェック / Rate Limit（Blueprint値 + `X-RateLimit-*` ヘッダー） / CORS / Logging / Request ID
- JWT発行・検証ユーティリティ
- AES-256-GCM 暗号化ユーティリティ（API Key用）
- Refresh Token 用 SQLite セットアップ

---

### Phase 2: Auth

- `POST /auth/register`（初回のみ、token pair 返却）
- `POST /auth/login`
- `POST /auth/refresh`（Refresh Token SQLite保存込み）
- `POST /auth/logout`（Refresh Token 無効化）
- `GET /auth/me`
- `PATCH /auth/me`（display_name, password 変更, avatar）

---

### Phase 3: Providers / Models

**SSE（Phase 6）の前提になるため早期実装**

- Provider CRUD（7 endpoints）
- API Key の暗号化保存・復号・マスク表示
- Model CRUD（6 endpoints under `/providers/{id}/models`、価格フィールド含む）
- `GET /models`（全Provider横断一覧、フィルタ対応）
- Model Capabilities CRUD（2 endpoints、オーバーライド対応）
- `POST /providers/{id}/test`（接続テスト・レイテンシ計測）
- `POST /providers/{id}/sync` / `POST /providers/{id}/models/sync`（同一Service）
- Provider Adapter インターフェース定義
  - OpenAI Compatible Adapter
  - OpenRouter Adapter

---

### Phase 4: Sessions

- Session CRUD（5 endpoints、詳細に統計情報含む）
- `DELETE /sessions`（一括削除、`{ ids: [] }`）
- `POST /sessions/{id}/duplicate`（複製）
- `GET /sessions/{id}/export`（format=json|md|txt） / `POST /sessions/import`（JSON/Markdown）
- InMemorySessionRepository

---

### Phase 5: Messages（非ストリーミング）

- Message CRUD（5 endpoints、カーソルページネーション）
- メッセージ編集の `regenerate_following` オプション
- `DELETE /sessions/{id}/messages/{msgId}/after`（以降削除）
- `POST /sessions/{id}/messages/{msgId}/regenerate`（新バージョン追加方式）
- `POST /sessions/{id}/messages/{msgId}/branch`
- `POST /sessions/merge`（`{ source_ids[], target_id, strategy }`）
- バージョン管理（`GET .../versions`, `PATCH .../version`、`active_version_id` モデル）
- Context Builder（コンテキストウィンドウ管理含む）
- `POST /sessions/{id}/messages` の非ストリーミング実装

---

### Phase 6: SSE

- `POST /sessions/{id}/messages?stream=true`（メイン送信ルート、SSE応答）
- `POST .../regenerate?stream=true`
- `SSE GET /sessions/{id}/stream`（購読チャネル）
- SSEイベント契約の実装
  - `message.start` / `content.delta` / `content.done` / `usage` / `message.stop` / `error`
  - `tool.start` / `tool.result`（Phase 11 で実際に動作）
- Streaming Adapter（Provider Adapter のストリーミング対応）
- 切断検知・部分保存（status: "interrupted"）
- SSE 同時接続制限（セッションあたり1接続）

---

### Phase 7: Attachments

- `POST /sessions/{id}/attachments`（multipart アップロード）
- `GET /sessions/{id}/attachments`（一覧）
- `GET /sessions/{id}/attachments/{attachId}`（メタデータ / ?download=true）
- `DELETE /sessions/{id}/attachments/{attachId}`（削除）
- ファイルサイズ（10MB / 413）・MIMEタイプバリデーション
- ローカル保存（`/data/attachments/`）
- メッセージコンテキストへの展開（テキスト・画像・PDF）

---

### Phase 8: Settings

- `GET/PATCH /settings`（全体。shortcuts / api_usage / tools / mcp / sandbox 含む）
- `GET/PATCH /settings/{category}`（appearance, chat, notifications, privacy）
- MCP config 保存（`settings.mcp.servers[]`）
- `POST /settings/reset`（`{ category?: string }` 対応）
- `GET /settings/export`（API キー除外） / `POST /settings/import`

---

### Phase 9: Folders / Tags / Presets

- Folder CRUD（4 endpoints、`cascade=true` 削除・color/icon 対応）
- Tag 系（4 endpoints、`{ tags: string[] }` 複数追加・使用頻度集計）
- Preset CRUD（5 endpoints）
- `POST /presets/{id}/apply`（`override_model` 対応）

---

### Phase 10: Search

- `GET /search`（横断検索、`type/from/to/model_id` フィルタ）
- `GET /search/sessions`（タイトル・システムプロンプト）
- `GET /search/messages`（スニペット付き）
- インメモリ期間: 文字列マッチング（case-insensitive）
- SQLite移行後: FTS5 全文検索

---

### Phase 11: Tools / MCP

- ToolRegistry（内部 Tool 管理）
- SearXNG Search Tool（Web検索）
- Web Fetch Tool
- McpClientManager（stdio プロセス管理）
- MCP Tool 実行フロー（ラウンドトリップ）
- Tool ラウンドトリップ上限（最大10回）
- SSE `tool.start` / `tool.result` イベント実装
- Artifact Tool
- Sandbox Trait（実 Provider は後日）

---

### Phase 12: System Endpoints

- `GET /health`（public、DB接続・MCP接続確認）
- `GET /version`（**public**、バージョン・ビルド情報・対応機能リスト）
- `GET /status`（認証必要、Provider稼働状況・レスポンスタイム）
- `GET /metrics`（admin専用、トークン数・コスト・リクエスト数・エラーレート）

---

### Phase 13: SQLite 移行

- InMemory → SQLite の Repository 差し替え
- マイグレーション管理（バージョン管理テーブル）
- 既存データの移行スクリプト（必要な場合）
- FTS5 全文検索インデックスの設定
- 全 Repository のテスト

---

## 検証フロー

実装完了後に以下の順序で通し確認する:

```
1.  初回 /auth/register（email, password, display_name）で admin 作成
    → レスポンスに access/refresh token が含まれること（自動ログイン）
2.  2回目の /auth/register が 403 になること
3.  /auth/login で Access Token + Refresh Token 取得
4.  Bearer なしで保護エンドポイントが 401 になること
5.  Refresh Token で Access Token 再発行（ローテーション確認）
6.  サーバー再起動後も Refresh Token が有効なこと
7.  /auth/logout で Refresh Token が無効化されること
8.  Rate Limit 超過時に 429 + X-RateLimit-* ヘッダーが返ること
9.  Provider を追加（API Key 保存、レスポンスでマスク確認）
10. /providers/{id}/test で接続確認（latency_ms 含む）
11. /providers/{id}/sync でモデル一覧取得
12. Model の価格フィールド・Capabilities を更新
13. Session 作成（folder_id, preset_id 指定）
14. 非ストリーミングでメッセージ送信・応答受信
15. POST /messages?stream=true で SSE delta 受信
    （message.start → content.delta → content.done → usage → message.stop）
16. GET /sessions/{id}/stream の購読チャネル動作確認
17. SSE 切断 → interrupted メッセージの保存確認
18. メッセージ一覧の before/after カーソルページネーション確認
19. メッセージ編集 → バージョン履歴確認、regenerate_following の動作確認
20. バージョン切り替え（PATCH .../version）
21. Regenerate で新バージョン追加（旧バージョンが versions に残ること）
22. Regenerate?stream=true の SSE 確認
23. Branch で新セッション作成
24. Merge（source_ids → target_id、strategy 指定）で統合
25. DELETE /sessions/{id}/messages/{msgId}/after で以降削除
26. DELETE /sessions { ids: [...] } で一括削除
27. Export format=json|md|txt の3形式確認、Import（JSON/Markdown）の往復確認
28. Preset 作成 → Apply（override_model: false でモデル維持確認）
29. 添付ファイルアップロード（11MB で 413 確認）→ メッセージコンテキストに反映
30. Folder cascade=true 削除の確認
31. MCP Server 設定 → チャット内 tool 実行
32. Tool ラウンドトリップの上限確認
33. /search でセッション・メッセージ検索（from/to フィルタ含む）
34. /settings export（API キーが含まれないこと）→ import の往復確認
35. /settings/reset { category: "appearance" } でカテゴリ単位リセット確認
36. /health, /version が未認証でアクセス可能なこと
37. /metrics が admin 以外に 403 を返すこと
38. SQLite 移行後、全データが引き継がれること
39. FTS5 全文検索が動作すること
```

---

## 保留事項（後回しでよいもの）


| 項目                                    | 理由                                              |
| --------------------------------------- | ------------------------------------------------- |
| Sandbox Docker 実装                     | Provider が未決定                                 |
| React Artifact                          | ArtifactKind 拡張で後から追加可能                 |
| OpenRouter Adapter 以外の Adapter       | OpenAI Compatible で代替可能                      |
| SSE Last-Event-ID による再開            | 初期は「中断・部分保存」で十分                    |
| セッション内マルチブランチ（branch_id） | ブランチ＝新セッション方式で当面カバー            |
| 論理削除（ゴミ箱）                      | 物理削除で開始。`deleted_at` 追加で後から移行可能 |
| 複数管理者                              | 単一管理者前提のため不要                          |
| HTTPS / TLS                             | デプロイ時にリバースプロキシで対応                |
| WebSocket への移行                      | SSE で十分。将来必要になれば検討                  |

---

## Open Questions

> [!IMPORTANT]
> 以下の点について確認が必要です。

1. **`/auth/register` の `email` vs `username`**: Blueprint は `email`。ローカルツールとして
   `username` の方がシンプルだが、Blueprint 契約を優先するなら `email`。
   → **推奨: Blueprint どおり `email`**（形式バリデーションは緩めでよい）。
2. **添付ファイルの `message_id` 紐付けタイミング**: アップロードはセッション単位、
   メッセージ送信時に `attachment_ids` で紐付ける方式で問題ないか。
   → 未紐付けのまま残った添付の掃除（セッション削除時 or 定期クリーンアップ）も決めておく。
3. **Merge の `strategy` の種類**: Blueprint は `strategy` の値を定義していない。
   `"chronological"`（時系列） / `"append"`（連結）の2種で開始し、必要に応じて拡張でよいか。
4. **`branch_id` の扱い**: メッセージ一覧クエリに `branch_id` があるが、
   ブランチ＝新セッション方式なら未使用。受け取って無視する実装で問題ないか。

### 解決済み（v4 でクローズ）

- ~~SSE の `message` パラメータ長制限~~ → `POST /messages?stream=true` が Blueprint の正規ルートであるため解消。
  `GET /sessions/{id}/stream` は購読チャネルとして併存させる。

## 補足

- **v3 の判断で維持してよいもの**: Q1〜Q3 の決定事項、JWT/Refresh Token 戦略、AES-256-GCM、レイヤー構成、Phase 分割と依存グラフは妥当です。そのまま v4 に引き継ぎました。
- **最も影響が大きい修正は #1（`stream=true` の復活）と #4（Merge の仕様）** です。これらは Phase 5/6 の設計に直結するため、実装前に必ず確定させてください。
- Open Questions の 3・4（Merge の strategy 値、`branch_id` の扱い）は Blueprint 側に値の定義がないため、実装前にあなたの側で決定が必要です。

> Generated by
> - Nex N2 Pro
> - Claude 4.6 Sonnet
> - Claude 4.6 Opus
> - Claude Fable 5
> - Kimi k2.6
> - and me :)