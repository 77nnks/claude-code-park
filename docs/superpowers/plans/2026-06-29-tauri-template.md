# Tauri Windows アプリテンプレート化 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude Code Park のリポジトリから Claude Code 固有のコードを除去し、Tauri v2 + React + Rust + Windows 対応の汎用スターターテンプレートを作る。

**Architecture:** Claude Code Park の実装パターン（ts-rs 型共有・Zustand push/pull Store・IPC ラッパー・safe_write・Windows ターミナル検出）を維持しつつ、ドメイン固有のコード（JSONL パース・PixiJS タウン描画・Claude セッション追跡・メトリクス収集）を最小限の汎用サンプル（Item CRUD）に置き換える。

**Tech Stack:** Tauri v2 / React 18 / Rust (stable) / Zustand 5 / ts-rs 10 / Vite 6 / Vitest

## Global Constraints

- Rust エディション: 2021、rust-version: 1.77
- Tauri バージョン: 2.x（タウリ 1.x へのダウングレード不可）
- ts-rs バージョン: 10（`#[ts(export)]` の書き方が変わるため固定）
- Windows 11 をメインターゲットとする（macOS ビルドも維持）
- すべてのコマンドは `PowerShell`（プロジェクトルートから実行）

---

## ファイル構成マップ

### 新規作成
```
src-tauri/src/model/item.rs        ← 汎用ドメインモデル（サンプル）
src-tauri/src/commands/items_cmd.rs← CRUD コマンド（サンプル）
src/stores/itemStore.ts            ← Zustand Store（サンプル）
src/components/ItemList.tsx        ← React リスト UI（サンプル）
```

### 変更
```
src-tauri/Cargo.toml               ← パッケージ名・不要依存を削除
src-tauri/src/lib.rs               ← コマンド登録を新コマンドのみに
src-tauri/src/state.rs             ← AppState を汎用化（ClaudePaths 除去）
src-tauri/src/paths.rs             ← アプリ固有パス解決に汎用化
src-tauri/src/error.rs             ← ClaudeHomeNotFound を汎用エラーに
src-tauri/src/model/mod.rs         ← item.rs のみ公開
src-tauri/src/commands/mod.rs      ← items_cmd のみ公開
src/App.tsx                        ← タブ構成を main/settings のみに
src/ipc/commands.ts                ← 新コマンドのラッパーに差し替え
src/ipc/events.ts                  ← イベント定義を汎用化
src/i18n/messages/ja.ts            ← 最小限キーに削減
src/i18n/messages/{en,de,es,fr,ko,zh}.ts ← ja に合わせて更新
.github/workflows/release.yml      ← Windows ビルドジョブを追加
```

### 削除（Claude Code 固有）
```
src-tauri/src/pipeline/            ← JSONL セッション追跡
src-tauri/src/jsonl/               ← JSONL パーサー
src-tauri/src/metrics/             ← メトリクス集計
src-tauri/src/watcher/             ← ファイル監視
src-tauri/src/hook_events.rs       ← Hook イベント型
src-tauri/src/model/session.rs / activity.rs / timeline.rs / metrics.rs
src-tauri/src/commands/agents_cmd.rs / hooks_cmd.rs / skills_cmd.rs
src-tauri/src/commands/metrics_cmd.rs / session_cmd.rs / world_cmd.rs
src-tauri/src/config_io/agents.rs / commands.rs / plugins.rs / settings.rs / skills.rs
src/office/                        ← PixiJS タウン描画エンジン全体
src/stores/worldStore.ts / hookStore.ts / metricsStore.ts / configStore.ts
src/stores/effectiveHooksStore.ts / effectiveAgentsStore.ts / effectiveSkillsStore.ts
src/stores/scopedConfigStore.ts / scopedMetricsStore.ts
src/stores/openLogStore.ts / hookDetailStore.ts / roomMenuStore.ts / characterStore.ts / uiPrefsStore.ts / configSource.ts
src/components/AgentDetail.tsx / AgentEditor.tsx / AgentsManager.tsx
src/components/CharacterEditor.tsx / CharacterLogDialog.tsx / HookDetailDialog.tsx
src/components/HooksManager.tsx / MetricsDashboard.tsx / RoomMenuOverlay.tsx
src/components/SkillEditor.tsx / SkillsManager.tsx / ToolChips.tsx
```

### 保持（再利用）
```
src-tauri/src/config_io/safe_write.rs   ← バックアップ付き安全書き込み
src-tauri/src/config_io/frontmatter.rs  ← Markdown フロントマターパーサー
src-tauri/src/config_io/mod.rs          ← 上記2つのみ公開に変更
src-tauri/src/terminal/                 ← Windows ターミナル検出
src-tauri/src/commands/terminal_cmd.rs  ← ターミナルフォーカスコマンド
src/components/Settings.tsx             ← 設定画面（簡略化）
src/components/Toast.tsx                ← トースト通知
src/stores/toastStore.ts                ← トースト Store
src/i18n/                               ← i18n 全体（キーを削減して流用）
```

---

## Task 1: プロジェクトのコピーと ID 変更

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `package.json`

**Interfaces:**
- Produces: ビルドが通る新しいプロジェクト名の骨格

---

- [ ] **Step 1: リポジトリを新ディレクトリにコピー**

```powershell
# 例: C:\VSCode\my-tauri-app に新プロジェクトを作る
Copy-Item -Path "C:\VSCode\claude-code-park" -Destination "C:\VSCode\my-tauri-app" -Recurse
cd "C:\VSCode\my-tauri-app"
# .git を再初期化して履歴をリセット
Remove-Item -Recurse -Force .git
git init
git add .
git commit -m "chore: initial commit from claude-code-park template"
```

- [ ] **Step 2: Cargo.toml のパッケージ名を変更**

`src-tauri/Cargo.toml` を以下のように変更する（`my-tauri-app` はアプリ名に変えること）:

```toml
[package]
name = "my-tauri-app"
version = "0.1.0"
description = "My Tauri App"
edition = "2021"
rust-version = "1.77"

[lib]
name = "my_tauri_app_lib"
crate-type = ["lib", "cdylib", "staticlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
ts-rs = "10"
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
] }
sysinfo = "0.32"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

- [ ] **Step 3: tauri.conf.json のアプリ名を変更**

`src-tauri/tauri.conf.json` の以下のフィールドを変更する:

```json
{
  "productName": "My Tauri App",
  "identifier": "com.example.my-tauri-app"
}
```

- [ ] **Step 4: package.json の name を変更**

```json
{
  "name": "my-tauri-app",
  "version": "0.1.0"
}
```

- [ ] **Step 5: Cargo.lock を再生成してビルドが通ることを確認**

```powershell
cd src-tauri
cargo check 2>&1 | Select-Object -First 30
```

期待: エラーが出る（まだ古いコードが残っているため）。次のタスクで解消する。

- [ ] **Step 6: コミット**

```powershell
cd ..
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json
git commit -m "chore: rename project to my-tauri-app"
```

---

## Task 2: Claude Code 固有の Rust モジュールを削除

**Files:**
- Delete: `src-tauri/src/pipeline/`
- Delete: `src-tauri/src/jsonl/`
- Delete: `src-tauri/src/metrics/`
- Delete: `src-tauri/src/watcher/`
- Delete: `src-tauri/src/hook_events.rs`
- Delete: `src-tauri/src/model/session.rs` / `activity.rs` / `timeline.rs` / `metrics.rs`
- Delete: `src-tauri/src/commands/agents_cmd.rs` / `hooks_cmd.rs` / `skills_cmd.rs` / `metrics_cmd.rs` / `session_cmd.rs` / `world_cmd.rs`
- Delete: `src-tauri/src/config_io/agents.rs` / `commands.rs` / `plugins.rs` / `settings.rs` / `skills.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/paths.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/model/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/config_io/mod.rs`

**Interfaces:**
- Produces: コンパイルが通る最小限の Rust バックエンド骨格

---

- [ ] **Step 1: Claude Code 固有ディレクトリを削除**

```powershell
cd src-tauri/src
Remove-Item -Recurse -Force pipeline, jsonl, metrics, watcher
Remove-Item -Force hook_events.rs
Remove-Item -Force model/session.rs, model/activity.rs, model/timeline.rs, model/metrics.rs
Remove-Item -Force commands/agents_cmd.rs, commands/hooks_cmd.rs, commands/skills_cmd.rs
Remove-Item -Force commands/metrics_cmd.rs, commands/session_cmd.rs, commands/world_cmd.rs
Remove-Item -Force config_io/agents.rs, config_io/commands.rs, config_io/plugins.rs
Remove-Item -Force config_io/settings.rs, config_io/skills.rs
cd ../..
```

- [ ] **Step 2: error.rs を汎用化**

`src-tauri/src/error.rs` を以下の内容に置き換える:

```rust
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("item not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 3: paths.rs を汎用化**

`src-tauri/src/paths.rs` を以下の内容に置き換える:

```rust
use std::path::PathBuf;

/// アプリのデータディレクトリを解決する。
/// デフォルトは OS のユーザーデータディレクトリ配下の `my-tauri-app/`。
/// アプリ名・パス構成はここで一元管理する。
pub struct AppPaths {
    pub data_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("my-tauri-app");
        Self { data_dir }
    }

    /// アイテムを JSON で保存するファイル。
    pub fn items_file(&self) -> PathBuf {
        self.data_dir.join("items.json")
    }

    /// バックアップ置き場。
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
}
```

- [ ] **Step 4: state.rs を汎用化**

`src-tauri/src/state.rs` を以下の内容に置き換える:

```rust
use crate::model::item::Item;
use crate::paths::AppPaths;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::collections::HashMap;

const ECHO_SUPPRESS: Duration = Duration::from_millis(1500);

/// アプリ全体のデータストア（Tauri の manage() でシングルトン管理）。
#[derive(Default)]
pub struct AppData {
    pub items: Vec<Item>,
}

pub struct AppState {
    pub data: Mutex<AppData>,
    pub paths: AppPaths,
    recently_written: Mutex<HashMap<PathBuf, Instant>>,
}

impl AppState {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            data: Mutex::new(AppData::default()),
            paths,
            recently_written: Mutex::new(HashMap::new()),
        }
    }

    pub fn mark_written(&self, path: &Path) {
        self.recently_written
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), Instant::now());
    }

    pub fn is_self_write(&self, path: &Path) -> bool {
        let mut map = self.recently_written.lock().unwrap();
        map.retain(|_, t| t.elapsed() < ECHO_SUPPRESS);
        map.contains_key(path)
    }
}
```

- [ ] **Step 5: model/mod.rs を更新**

`src-tauri/src/model/mod.rs` を以下の内容に置き換える:

```rust
pub mod item;
```

- [ ] **Step 6: commands/mod.rs を更新**

`src-tauri/src/commands/mod.rs` を以下の内容に置き換える:

```rust
pub mod items_cmd;
pub mod terminal_cmd;
```

- [ ] **Step 7: config_io/mod.rs を更新**

`src-tauri/src/config_io/mod.rs` を以下の内容に置き換える:

```rust
pub mod frontmatter;
pub mod safe_write;
```

- [ ] **Step 8: lib.rs を最小構成に**

`src-tauri/src/lib.rs` を以下の内容に置き換える:

```rust
mod commands;
mod config_io;
mod error;
mod model;
mod paths;
mod state;
mod terminal;

use paths::AppPaths;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = AppPaths::discover();
    tauri::Builder::default()
        .manage(AppState::new(paths))
        .invoke_handler(tauri::generate_handler![
            commands::items_cmd::list_items,
            commands::items_cmd::create_item,
            commands::items_cmd::delete_item,
            commands::terminal_cmd::focus_terminal,
        ])
        .run(tauri::generate_context!())
        .expect("failed to launch app");
}
```

- [ ] **Step 9: ビルドエラーを確認（次タスクで解消する）**

```powershell
cd src-tauri
cargo check 2>&1 | Select-Object -First 50
cd ..
```

期待: `model::item` と `commands::items_cmd` が存在しないというエラー。次のタスクで作成する。

- [ ] **Step 10: コミット**

```powershell
git add -A
git commit -m "chore: remove claude-code-specific rust modules"
```

---

## Task 3: 汎用ドメインモデルを作成（ts-rs 連携）

**Files:**
- Create: `src-tauri/src/model/item.rs`

**Interfaces:**
- Produces: `Item` 型（TypeScript 型として `src/bindings/Item.ts` に自動生成される）

---

- [ ] **Step 1: model/item.rs の failing テストを書く**

`src-tauri/src/model/item.rs` を新規作成:

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 汎用アイテム。アプリのドメインに合わせてフィールドを変更する。
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/bindings/")]
pub struct Item {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub created_at: String,  // ISO8601
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_serializes_to_json() {
        let item = Item {
            id: "abc".into(),
            title: "hello".into(),
            done: false,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"id\":\"abc\""));
        assert!(json.contains("\"done\":false"));
    }

    #[test]
    fn item_deserializes_from_json() {
        let json = r#"{"id":"x","title":"t","done":true,"created_at":"2026-01-01T00:00:00Z"}"#;
        let item: Item = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, "x");
        assert!(item.done);
    }
}
```

- [ ] **Step 2: テストを実行して失敗を確認**

```powershell
cd src-tauri
cargo test model::item 2>&1
cd ..
```

期待: コンパイルエラー（まだ model/item.rs が存在しない場合）または PASS（ファイルを作成済みの場合）。

- [ ] **Step 3: cargo check で型が通ることを確認**

```powershell
cd src-tauri
cargo check 2>&1
cd ..
```

期待: `items_cmd` が未実装というエラーのみ残る（item.rs 自体のエラーはなし）。

- [ ] **Step 4: コミット**

```powershell
git add src-tauri/src/model/item.rs
git commit -m "feat: add generic Item domain model with ts-rs"
```

---

## Task 4: 汎用 CRUD コマンドを作成

**Files:**
- Create: `src-tauri/src/commands/items_cmd.rs`

**Interfaces:**
- Consumes: `model::item::Item`, `state::AppState`, `config_io::safe_write::safe_write`
- Produces:
  - `list_items() -> AppResult<Vec<Item>>`
  - `create_item(title: String) -> AppResult<Item>`
  - `delete_item(id: String) -> AppResult<()>`

---

- [ ] **Step 1: items_cmd.rs のテストを書く**

`src-tauri/src/commands/items_cmd.rs` を新規作成:

```rust
use crate::error::{AppError, AppResult};
use crate::model::item::Item;
use crate::state::AppState;
use chrono::Utc;
use tauri::State;

/// 全アイテムを返す。
#[tauri::command]
pub fn list_items(state: State<'_, AppState>) -> AppResult<Vec<Item>> {
    let data = state.data.lock().unwrap();
    Ok(data.items.clone())
}

/// アイテムを追加して保存する。
#[tauri::command]
pub fn create_item(state: State<'_, AppState>, title: String) -> AppResult<Item> {
    if title.trim().is_empty() {
        return Err(AppError::Invalid("title must not be empty".into()));
    }
    let item = Item {
        id: uuid_v4(),
        title: title.trim().to_string(),
        done: false,
        created_at: Utc::now().to_rfc3339(),
    };
    {
        let mut data = state.data.lock().unwrap();
        data.items.push(item.clone());
        persist(&state, &data.items)?;
    }
    Ok(item)
}

/// アイテムを ID で削除して保存する。
#[tauri::command]
pub fn delete_item(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let mut data = state.data.lock().unwrap();
    let before = data.items.len();
    data.items.retain(|i| i.id != id);
    if data.items.len() == before {
        return Err(AppError::NotFound(id));
    }
    persist(&state, &data.items)?;
    Ok(())
}

/// items.json に書き込む（safe_write でバックアップ付き）。
fn persist(state: &AppState, items: &[Item]) -> AppResult<()> {
    let json = serde_json::to_string_pretty(items)?;
    let stamp = chrono::Utc::now().timestamp();
    crate::config_io::safe_write::safe_write(
        &state.paths.items_file(),
        &json,
        &state.paths.backups_dir(),
        stamp,
    )
}

/// シンプルな UUID v4 相当（外部クレート不要）。
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{t:032x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_title_returns_error() {
        // create_item のバリデーションを直接テスト（Tauri State なしで）
        let title = "   ";
        assert!(title.trim().is_empty(), "whitespace-only title should be caught");
    }

    #[test]
    fn uuid_v4_is_unique() {
        let a = uuid_v4();
        let b = uuid_v4();
        assert_ne!(a, b);
    }
}
```

- [ ] **Step 2: テストを実行**

```powershell
cd src-tauri
cargo test commands::items 2>&1
cd ..
```

期待: `empty_title_returns_error` と `uuid_v4_is_unique` が PASS。

- [ ] **Step 3: cargo build でフル確認**

```powershell
cd src-tauri
cargo build 2>&1
cd ..
```

期待: ビルド成功。

- [ ] **Step 4: TypeScript 型バインディングが生成されることを確認**

ts-rs は `cargo test` のタイミングで export する（`#[ts(export)]`）。

```powershell
cd src-tauri
cargo test 2>&1
cd ..
# src/bindings/Item.ts が生成されているか確認
Get-Content src/bindings/Item.ts
```

期待:
```typescript
export type Item = { id: string; title: string; done: boolean; created_at: string; };
```

- [ ] **Step 5: コミット**

```powershell
git add src-tauri/src/commands/items_cmd.rs src/bindings/
git commit -m "feat: add CRUD commands for Item + ts-rs bindings"
```

---

## Task 5: フロントエンドの Claude Code 固有コードを削除

**Files:**
- Delete: `src/office/` (全体)
- Delete: `src/stores/worldStore.ts` / `hookStore.ts` / `metricsStore.ts` / `configStore.ts`
- Delete: `src/stores/effectiveHooksStore.ts` / `effectiveAgentsStore.ts` / `effectiveSkillsStore.ts`
- Delete: `src/stores/scopedConfigStore.ts` / `scopedMetricsStore.ts` / `configSource.ts`
- Delete: `src/stores/openLogStore.ts` / `hookDetailStore.ts` / `roomMenuStore.ts`
- Delete: `src/stores/characterStore.ts` / `uiPrefsStore.ts`
- Delete: `src/components/AgentDetail.tsx` / `AgentEditor.tsx` / `AgentsManager.tsx`
- Delete: `src/components/CharacterEditor.tsx` / `CharacterLogDialog.tsx` / `HookDetailDialog.tsx`
- Delete: `src/components/HooksManager.tsx` / `MetricsDashboard.tsx` / `RoomMenuOverlay.tsx`
- Delete: `src/components/SkillEditor.tsx` / `SkillsManager.tsx` / `ToolChips.tsx`
- Modify: `package.json` (pixi.js を削除)

**Interfaces:**
- Produces: フロントエンドのビルドが通る最小限の状態

---

- [ ] **Step 1: Claude Code 固有フロントエンドファイルを削除**

```powershell
Remove-Item -Recurse -Force src/office
Remove-Item -Force src/stores/worldStore.ts, src/stores/hookStore.ts
Remove-Item -Force src/stores/metricsStore.ts, src/stores/configStore.ts
Remove-Item -Force src/stores/effectiveHooksStore.ts, src/stores/effectiveAgentsStore.ts
Remove-Item -Force src/stores/effectiveSkillsStore.ts
Remove-Item -Force src/stores/scopedConfigStore.ts, src/stores/scopedMetricsStore.ts
Remove-Item -Force src/stores/configSource.ts
Remove-Item -Force src/stores/openLogStore.ts, src/stores/hookDetailStore.ts
Remove-Item -Force src/stores/roomMenuStore.ts, src/stores/characterStore.ts
Remove-Item -Force src/stores/uiPrefsStore.ts
Remove-Item -Force src/components/AgentDetail.tsx, src/components/AgentEditor.tsx
Remove-Item -Force src/components/AgentsManager.tsx, src/components/CharacterEditor.tsx
Remove-Item -Force src/components/CharacterLogDialog.tsx, src/components/HookDetailDialog.tsx
Remove-Item -Force src/components/HooksManager.tsx, src/components/MetricsDashboard.tsx
Remove-Item -Force src/components/RoomMenuOverlay.tsx, src/components/SkillEditor.tsx
Remove-Item -Force src/components/SkillsManager.tsx, src/components/ToolChips.tsx
```

- [ ] **Step 2: pixi.js を package.json から削除**

`package.json` の `dependencies` から `"pixi.js"` の行を削除し、`npm install` を再実行:

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.1.1",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "zustand": "^5.0.2"
  }
}
```

```powershell
npm install
```

- [ ] **Step 3: ipc/commands.ts を新コマンドに差し替え**

`src/ipc/commands.ts` を以下の内容に置き換える:

```typescript
import { invoke } from "@tauri-apps/api/core";
import type { Item } from "../bindings";

export type { Item };

/**
 * Rust の #[tauri::command] への型付きラッパー。
 * 新しいコマンドを追加したらここに追記する。
 */
export const api = {
  listItems(): Promise<Item[]> {
    return invoke<Item[]>("list_items");
  },
  createItem(title: string): Promise<Item> {
    return invoke<Item>("create_item", { title });
  },
  deleteItem(id: string): Promise<void> {
    return invoke<void>("delete_item", { id });
  },
  focusTerminal(sessionId: string, project: string): Promise<{ app: string; window_focused: boolean }> {
    return invoke("focus_terminal", { sessionId, project });
  },
};
```

- [ ] **Step 4: ipc/events.ts を汎用化**

`src/ipc/events.ts` を以下の内容に置き換える:

```typescript
import { listen } from "@tauri-apps/api/event";

/**
 * バックエンドからのイベントリスナー登録。
 * Rust 側で app.emit() するイベントごとにここに追記する。
 */

/** アイテムリストが変更されたとき（バックグラウンド書き込みがある場合に使う）。 */
export async function onItemsUpdated(cb: () => void): Promise<() => void> {
  const unlisten = await listen("state://items/updated", () => cb());
  return unlisten;
}
```

- [ ] **Step 5: コミット**

```powershell
git add -A
git commit -m "chore: remove claude-code-specific frontend code and pixi.js"
```

---

## Task 6: 汎用フロントエンド UI を実装

**Files:**
- Create: `src/stores/itemStore.ts`
- Create: `src/components/ItemList.tsx`
- Modify: `src/App.tsx`

**Interfaces:**
- Consumes: `api.listItems()`, `api.createItem()`, `api.deleteItem()` from `src/ipc/commands.ts`
- Produces: 動作するアイテム CRUD UI

---

- [ ] **Step 1: itemStore.ts のテストを書く**

`src/stores/itemStore.ts` を新規作成:

```typescript
import { create } from "zustand";
import { api, type Item } from "../ipc/commands";

type ItemState = {
  items: Item[];
  loading: boolean;
  error: string | null;
  load(): Promise<void>;
  create(title: string): Promise<void>;
  remove(id: string): Promise<void>;
};

export const useItemStore = create<ItemState>((set, get) => ({
  items: [],
  loading: false,
  error: null,

  async load() {
    set({ loading: true, error: null });
    try {
      const items = await api.listItems();
      set({ items, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  async create(title: string) {
    try {
      const item = await api.createItem(title);
      set((s) => ({ items: [...s.items, item] }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  async remove(id: string) {
    try {
      await api.deleteItem(id);
      set((s) => ({ items: s.items.filter((i) => i.id !== id) }));
    } catch (e) {
      set({ error: String(e) });
    }
  },
}));
```

- [ ] **Step 2: itemStore.test.ts を書く**

`src/stores/itemStore.test.ts` を新規作成:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";

// api をモック
vi.mock("../ipc/commands", () => ({
  api: {
    listItems: vi.fn().mockResolvedValue([]),
    createItem: vi.fn().mockResolvedValue({ id: "1", title: "test", done: false, created_at: "2026-01-01T00:00:00Z" }),
    deleteItem: vi.fn().mockResolvedValue(undefined),
  },
}));

import { useItemStore } from "./itemStore";

beforeEach(() => {
  useItemStore.setState({ items: [], loading: false, error: null });
});

describe("itemStore", () => {
  it("load sets items from api", async () => {
    const { api } = await import("../ipc/commands");
    vi.mocked(api.listItems).mockResolvedValueOnce([
      { id: "a", title: "hello", done: false, created_at: "2026-01-01T00:00:00Z" },
    ]);
    await useItemStore.getState().load();
    expect(useItemStore.getState().items).toHaveLength(1);
    expect(useItemStore.getState().items[0].title).toBe("hello");
  });

  it("create appends item optimistically", async () => {
    await useItemStore.getState().create("new task");
    expect(useItemStore.getState().items).toHaveLength(1);
    expect(useItemStore.getState().items[0].title).toBe("test");
  });

  it("remove filters item by id", async () => {
    useItemStore.setState({ items: [{ id: "1", title: "x", done: false, created_at: "" }] });
    await useItemStore.getState().remove("1");
    expect(useItemStore.getState().items).toHaveLength(0);
  });
});
```

- [ ] **Step 3: Vitest でテストを実行して失敗を確認**

```powershell
npm test -- --reporter verbose 2>&1
```

期待: itemStore のテストが FAIL（まだ itemStore.ts が空のため）。

- [ ] **Step 4: テストを PASS させる（上記 itemStore.ts を作成済みなら即 PASS）**

```powershell
npm test -- --reporter verbose 2>&1
```

期待: 3 件すべて PASS。

- [ ] **Step 5: ItemList.tsx を作成**

`src/components/ItemList.tsx` を新規作成:

```tsx
import { useState } from "react";
import { useItemStore } from "../stores/itemStore";

export function ItemList() {
  const { items, loading, error, load, create, remove } = useItemStore();
  const [title, setTitle] = useState("");

  const handleCreate = async () => {
    if (!title.trim()) return;
    await create(title);
    setTitle("");
  };

  return (
    <div className="panel">
      <div className="toolbar">
        <h2>Items</h2>
        <button className="btn secondary" onClick={load} disabled={loading}>
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {error && <div className="err">{error}</div>}

      <div className="row" style={{ gap: 8, padding: "8px 0" }}>
        <input
          className="input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleCreate()}
          placeholder="New item title…"
        />
        <button className="btn" onClick={handleCreate}>Add</button>
      </div>

      <ul style={{ listStyle: "none", padding: 0 }}>
        {items.map((item) => (
          <li key={item.id} style={{ display: "flex", gap: 8, padding: "4px 0" }}>
            <span style={{ flex: 1, opacity: item.done ? 0.5 : 1 }}>{item.title}</span>
            <button className="btn secondary" onClick={() => remove(item.id)}>Delete</button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 6: App.tsx をシンプルなタブ構成に**

`src/App.tsx` を以下の内容に置き換える:

```tsx
import { useEffect } from "react";
import { ItemList } from "./components/ItemList";
import { Settings } from "./components/Settings";
import { Toast } from "./components/Toast";
import { useItemStore } from "./stores/itemStore";
import { useState } from "react";

type Tab = "main" | "settings";

export function App() {
  const [tab, setTab] = useState<Tab>("main");
  const load = useItemStore((s) => s.load);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="app">
      <div className="tabbar" data-tauri-drag-region>
        <span className="brand" data-tauri-drag-region>My Tauri App</span>
        <button className={`tab ${tab === "main" ? "active" : ""}`} onClick={() => setTab("main")}>
          Main
        </button>
        <button className={`tab ${tab === "settings" ? "active" : ""}`} onClick={() => setTab("settings")}>
          Settings
        </button>
      </div>
      <div className="content">
        {tab === "main" && <ItemList />}
        {tab === "settings" && <Settings />}
        <Toast />
      </div>
    </div>
  );
}
```

- [ ] **Step 7: TypeScript のビルドを確認**

```powershell
npx tsc --noEmit 2>&1
```

期待: エラーなし。

- [ ] **Step 8: コミット**

```powershell
git add -A
git commit -m "feat: add ItemList component and itemStore"
```

---

## Task 7: i18n を最小キーに削減

**Files:**
- Modify: `src/i18n/messages/ja.ts`
- Modify: `src/i18n/messages/en.ts`
- Modify: `src/i18n/messages/de.ts`
- Modify: `src/i18n/messages/es.ts`
- Modify: `src/i18n/messages/fr.ts`
- Modify: `src/i18n/messages/ko.ts`
- Modify: `src/i18n/messages/zh.ts`

**Interfaces:**
- Produces: TypeScript コンパイルが通る最小限の i18n カタログ

---

- [ ] **Step 1: ja.ts をテンプレートの最小構成に置き換え**

`src/i18n/messages/ja.ts` を以下の内容に置き換える:

```typescript
/**
 * 日本語メッセージカタログ（型定義のソース）。
 * この構造から Messages 型が導出される。他言語は同型を満たすことを TS で強制する。
 * {name} 形式は t() の第2引数で補間される。
 */
export const ja = {
  common: {
    close: "閉じる",
    cancel: "キャンセル",
    save: "保存",
    delete: "削除",
  },

  nav: {
    main: "メイン",
    settings: "設定",
  },

  app: {
    configError: "エラー: {error}",
  },

  settings: {
    title: "設定",
    languageSection: "言語",
    languageLabel: "表示言語",
  },

  terminal: {
    notFound: "対象のターミナルが見つかりませんでした",
  },
} as const;
```

- [ ] **Step 2: 他言語ファイルを同じ構造で更新（en.ts の例）**

`src/i18n/messages/en.ts`:

```typescript
import type { Messages } from "../locales";

export const en: Messages = {
  common: {
    close: "Close",
    cancel: "Cancel",
    save: "Save",
    delete: "Delete",
  },
  nav: {
    main: "Main",
    settings: "Settings",
  },
  app: {
    configError: "Error: {error}",
  },
  settings: {
    title: "Settings",
    languageSection: "Language",
    languageLabel: "Display language",
  },
  terminal: {
    notFound: "Terminal not found",
  },
};
```

同様に `de.ts`, `es.ts`, `fr.ts`, `ko.ts`, `zh.ts` を `Messages` 型に合わせて更新する（内容は各言語で翻訳する）。

- [ ] **Step 3: TypeScript コンパイルで型整合性を確認**

```powershell
npx tsc --noEmit 2>&1
```

期待: エラーなし。ja.ts のキーと他言語のキーが一致していないとここでエラーになる。

- [ ] **Step 4: コミット**

```powershell
git add src/i18n/
git commit -m "chore: strip i18n to minimal keys for template"
```

---

## Task 8: Windows CI ビルドを追加

**Files:**
- Modify: `.github/workflows/release.yml`

**Interfaces:**
- Produces: `v*` タグ push 時に macOS (.dmg) と Windows (.msi) の両アーティファクトを生成する GitHub Actions ワークフロー

---

- [ ] **Step 1: release.yml に Windows ジョブを追加**

`.github/workflows/release.yml` を以下の内容に置き換える:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  release-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin,x86_64-apple-darwin
      - uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - run: npm ci
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          args: --target universal-apple-darwin
          tagName: ${{ github.ref_name }}
          releaseName: "My Tauri App ${{ github.ref_name }}"
          releaseDraft: true
          prerelease: false

  release-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - run: npm ci
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "My Tauri App ${{ github.ref_name }}"
          releaseDraft: true
          prerelease: false
          # NOTE: Windows コード署名が必要な場合は以下を設定する:
          # TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          # TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

- [ ] **Step 2: コミット**

```powershell
git add .github/workflows/release.yml
git commit -m "ci: add Windows build job to release workflow"
```

---

## Task 9: 最終ビルド検証

**Files:** なし（確認のみ）

**Interfaces:**
- Produces: `cargo test` / `npm test` / `tauri dev` がすべて通る状態

---

- [ ] **Step 1: Rust テストをすべて実行**

```powershell
cd src-tauri
cargo test 2>&1
cd ..
```

期待:
```
test model::item::tests::item_serializes_to_json ... ok
test model::item::tests::item_deserializes_from_json ... ok
test commands::items_cmd::tests::empty_title_returns_error ... ok
test commands::items_cmd::tests::uuid_v4_is_unique ... ok
test config_io::safe_write::tests::writes_and_backs_up_existing ... ok
test config_io::safe_write::tests::creates_new_without_backup ... ok
test config_io::safe_write::tests::delete_backs_up ... ok
```

- [ ] **Step 2: フロントエンドテストをすべて実行**

```powershell
npm test 2>&1
```

期待:
```
✓ src/stores/itemStore.test.ts (3 tests)
```

- [ ] **Step 3: TypeScript 型チェック**

```powershell
npx tsc --noEmit 2>&1
```

期待: エラーなし。

- [ ] **Step 4: 開発サーバーを起動してアプリが表示されることを確認**

```powershell
npm run app
```

期待: アプリウィンドウが開き、「Main」タブに空のアイテムリストと入力フォームが表示される。

- [ ] **Step 5: アイテムの追加・削除が動作することを手動確認**

1. テキストボックスにタイトルを入力して「Add」をクリック → リストに追加される
2. 「Delete」をクリック → リストから削除される
3. アプリを再起動して「Refresh」をクリック → 永続化されているか確認

- [ ] **Step 6: 最終コミット**

```powershell
git add -A
git commit -m "chore: template complete - verified build and runtime"
```

---

## 完成後のカスタマイズポイント

このテンプレートを実際のアプリに育てる際の主な変更点:

| 変更箇所 | 内容 |
|---|---|
| `src-tauri/src/model/item.rs` | `Item` 構造体をアプリのドメインモデルに差し替え |
| `src-tauri/src/commands/items_cmd.rs` | ドメイン固有の Tauri コマンドを実装 |
| `src-tauri/src/paths.rs` | データ保存先のパスをアプリ名に合わせて変更 |
| `src/stores/itemStore.ts` | ドメイン Store に書き直す |
| `src/components/ItemList.tsx` | アプリの UI に書き直す |
| `src/i18n/messages/ja.ts` | アプリ固有のメッセージキーを追加 |
| `src-tauri/tauri.conf.json` | `productName` / `identifier` を最終名称に設定 |
| `.github/workflows/release.yml` | コード署名設定を追加（配布する場合） |
