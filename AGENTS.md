# nekoui
NekoUIは強力な機能と、Rust採用による高速な処理と安全性を生かしたAIチャットアプリです。

## 開発する際の鉄則
- クレートを追加する際は`nekoui`配下の`Cargo.toml`に追加して、`nekoui`配下にあるクレート内のCargo.tomlに`{追加したクレート}.workspace = true`とするようにしてください。

## クレート構造
###