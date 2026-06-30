# Claude Code Park — 仕様設計書

**作成日**: 2026-06-29  
**対象ブランチ**: feat/windows-port  
**目的**: 既存ソースコードから仕様を逆引きし、新規Windowsアプリ開発の参照資料とする

---

## 1. プロダクト概要

**Claude Code Park** は、Claude Code（Anthropic製CLIツール）のセッション活動をリアルタイムで可視化するデスクトップアプリケーションです。

### コンセプト

- `~/.claude/projects/` 以下のJSONLファイルをリアルタイムにテールし、各Claude Codeセッションの活動状態を等角投影（アイソメトリック）の「タウン」画面に投影する
- 1セッション = 1部屋。メインセッション = Orchestrator（社長キャラ）、サブエージェント = Employee（社員キャラ）
- タウン以外に Metrics / Agents / Hooks / Skills / Settings の管理画面を持つ

### 対象ユーザー

Claude Codeを日常的に使う開発者。複数セッション・複数エージェントを並列で使うヘビーユーザーを主なターゲットとする。

### 対応プラットフォーム

- **macOS**（メインリリース。マルチアーキテクチャバイナリ）
- **Windows 11**（feat/windows-port ブランチで移植中）

---

## 2. 技術スタック

| レイヤー | 技術 | バージョン | 役割 |
|---|---|---|---|
| デスクトップシェル | **Tauri v2** | ^2.1 | WebView+Rustブリッジ。IPC、ウィンドウ管理、ファイルシステムアクセス |
| フロントエンド | **React** | ^18.3 | UIコンポーネント |
| 状態管理 | **Zustand** | ^5.0 | Store（worldStore / hookStore / metricsStore 等） |
| 2Dレンダリング | **PixiJS** | ^8.6 | アイソメトリックタウン描画（WebGL/Canvas） |
| バックエンド | **Rust** (stable) | — | ファイル監視・パース・ビジネスロジック |
| 型共有 | **ts-rs** | — | Rustの型定義 → TypeScript型を自動生成（`src/bindings/`） |
| ビルドツール | **Vite** | ^6 | フロントエンドのバンドル |
| テスト | **Vitest** / `cargo test` | ^4 | TS側ユニットテスト / Rust側ユニットテスト |

### 依存関係の選定理由（転用時の参考）

- **Tauri v2を選んだ理由**: Rustバックエンドで`~/.claude`への直接ファイルアクセスが必要。Electronより軽量でバンドルサイズが小さい
- **PixiJSを選んだ理由**: アイソメトリック描画には高いフレームレートが必要。DOMベースのUIライブラリでは性能不足
- **ts-rsを選んだ理由**: Rust側のモデル変更がTypeScript側に即座に反映される。IPC境界での型ズレを防ぐ

---

## 3. アーキテクチャ全体図

### レイヤー構成

```
┌─────────────────────────────────────────────────────────┐
│  フロントエンド (React + PixiJS + Zustand)               │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │OfficeView│  │Metrics   │  │Agents/   │  │Settings│ │
│  │(PixiJS)  │  │Dashboard │  │Hooks/    │  │        │ │
│  └────┬─────┘  └────┬─────┘  │Skills    │  └────────┘ │
│       │              │        └────┬─────┘             │
│  ┌────▼──────────────▼─────────────▼─────────────────┐ │
│  │  Zustand Stores (worldStore / hookStore /          │ │
│  │  metricsStore / configStore / effectiveHooksStore) │ │
│  └─────────────────────┬──────────────────────────────┘ │
│                         │ api.invoke() / listen()        │
└─────────────────────────┼───────────────────────────────┘
                          │  Tauri IPC (invoke / emit)
┌─────────────────────────┼───────────────────────────────┐
│  バックエンド (Rust)     │                               │
│  ┌──────────────────────▼────────────────────────────┐  │
│  │  AppState { world: Mutex<World>, paths: ClaudePaths}│  │
│  └──────────────────────┬──────────────┬─────────────┘  │
│         ┌───────────────┘              │                  │
│  ┌──────▼─────────┐         ┌──────────▼────────────┐    │
│  │ watcher/       │         │ commands/             │    │
│  │ projects.rs    │         │ (Tauri #[command])    │    │
│  │ (notify +      │         │  world / agents /     │    │
│  │  TailReader)   │         │  hooks / skills /     │    │
│  └──────┬─────────┘         │  metrics / terminal   │    │
│         │                   └───────────────────────┘    │
│  ┌──────▼──────────────┐                                  │
│  │ pipeline/           │                                  │
│  │ session_tracker.rs  │                                  │
│  │ classify.rs         │                                  │
│  └──────┬──────────────┘                                  │
│         │                                                  │
│  ┌──────▼──────────────┐                                  │
│  │ jsonl/              │                                  │
│  │ tail.rs / parse.rs  │                                  │
│  └─────────────────────┘                                  │
└───────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────┐
│  ~/.claude/ (Claude Code が書き込むファイル群)            │
│  projects/{proj}/{sid}.jsonl                             │
│  projects/{proj}/{sid}/subagents/agent-{id}.jsonl        │
│  settings.json / agents/ / skills/ / commands/           │
│  sessions/{pid}.json (実行中セッションのPID情報)          │
└─────────────────────────────────────────────────────────┘
```

### 2本のデータフロー

**① Push型（リアルタイム監視）**

```
JSONL書き込み
  → notify debouncer (150ms)
    → TailReader.read_new()
      → pipeline::session_tracker::apply_*()
        → world.sessions 更新
          → app.emit("state://sessions/updated", sessions)
            → worldStore.sessions 更新
              → WorldRenderer.sync()
                → PixiJSスプライト更新
```

**② Pull型（コマンド呼び出し）**

```
フロントエンドのユーザー操作
  → api.invoke("get_metrics") 等
    → Rustの#[tauri::command]関数
      → AppState / config_io を参照
        → JSONレスポンス
          → Storeに格納 → UI再描画
```

### ライフサイクルイベントの流れ（Hook可視化）

```
JSONL entry (PreToolUse等)
  → reconstruct() でHookEventを生成
    → app.emit("state://lifecycle/fired", hook_event)
      → hookStore でflash記録
        → WorldRenderer.applyHooks()
          → スプライトのバッジ点灯 + HookBeamアニメーション
```

---

## 4. データモデル定義

> Rustで定義し、ts-rsで自動生成されたTypeScript型（`src/bindings/`）と共有される。

### セッション系

```
Session
├── session_id: String          // JSONL ファイル名（UUID）
├── project: String             // cwd（作業ディレクトリのフルパス）
├── git_branch: Option<String>
├── slug: Option<String>        // セッション名スラッグ
├── status: SessionStatus       // Active / Idle / Ended
├── started_at: Option<String>  // ISO8601
├── last_event_at: Option<String>
├── current: ActivityState      // 現在の作業状態
├── is_main: bool               // 常に true（メインセッション）
└── subagents: Vec<SubAgentRun>

SubAgentRun
├── agent_id: String            // agent-{id}.jsonl の id
├── subagent_type: Option<String> // Agent tool_use の subagent_type
├── description: Option<String>
├── model: Option<String>       // 実際に使用されたモデル（スプライト選択に使用）
├── started_at: Option<String>
├── status: SessionStatus
└── current: ActivityState

ActivityState
├── kind: WorkKind              // 現在の作業種別
├── tool_name: Option<String>   // 最後に呼ばれたツール名
├── detail: Option<String>      // 吹き出し表示用の詳細（ファイル名・コマンド等）
├── since: Option<String>       // この状態に入った時刻
├── active_skill: Option<String>// 現在実行中のスキル名
└── todos: Vec<TodoItem>        // TodoWriteで設定したタスクリスト

WorkKind（列挙）
  Idle / Thinking / Reading / Editing / Running /
  Searching / Reviewing / Delegating / WebExploring / AwaitingUser
```

### エージェント・設定系

```
AgentDef
├── name: String                // ファイル名 = フロントマターのname
├── description: String
├── tools: Vec<String>          // 使用可能ツール一覧
├── model: Option<String>
├── color: Option<String>
├── body: String                // フロントマター以降のMarkdown本文
├── file_path: String
└── source: AgentSource         // User / Project / Plugin(name)

SkillDef
├── name: String
├── description: String
├── disable_model_invocation: bool
├── argument_hint: Option<String>
├── allowed_tools: Vec<String>
├── disabled: bool              // SKILL.md.disabled の場合 true
├── body: String
├── dir: String                 // スキルディレクトリのパス
└── source: SkillSource         // User / Project / Plugin(name)

HookEntry
├── matcher: Option<String>     // ツール名の正規表現フィルタ
└── hooks: Vec<HookAction>

HookAction
├── action_type: String         // "command" / "agent" 等
├── command: Option<String>
└── extra: Map<String, Value>   // 上記以外のフィールドを保持（型不明なhookに対応）

ScopedHook                      // 有効フックの展開済み表現
├── scope: String               // "user" / "project" / "local" / "plugin"
├── matcher: Option<String>
├── command: String
└── plugin: Option<String>
```

### メトリクス系

```
AgentMetrics
├── agent_name: String          // "__main__" = メインセッション
└── windows: Map<String, MetricsWindow>
                                // "today" / "7d" / "30d"

MetricsWindow
├── active_seconds: u64
├── share: f64                  // 全エージェント合計に対する稼働割合 (0..1)
├── invocations: u32            // サブエージェント起動回数
├── tool_calls: u32
├── avg_tool_ms: f64
├── failure_rate: f64           // tool_result.is_error の割合 (0..1)
├── tokens_in: u64
└── tokens_out: u64
```

### パス解決（ClaudePaths）

```
~/.claude/
├── settings.json               // Hooks / 設定
├── agents/                     // AgentDef Markdownファイル群
├── skills/                     // SkillDef ディレクトリ群
├── commands/                   // CommandDef Markdownファイル群
├── projects/{proj}/            // {proj} = cwdのパス区切りを"-"に変換
│   ├── {sid}.jsonl             // メインセッションのJSONL
│   └── {sid}/subagents/        // サブエージェントのJSONL
│       └── agent-{aid}.jsonl
└── sessions/
    └── {pid}.json              // 実行中セッションのPID・セッションID情報
```

---

## 5. コアパイプライン（JSONLテール → 状態管理）

### 全体フロー

```
起動時
  ClaudePaths::discover()
    → ~/.claude/ が存在しない場合でも起動（空状態で表示）

  initial_scan()
    → glob("~/.claude/projects/**/*.jsonl")
    → 更新日時が6時間以内のファイルのみ全読み込み
    → それ以外は末尾にシーク（以降の追記のみ監視）

  watcher::spawn() → バックグラウンドスレッド
  watcher::config::spawn() → 設定ファイル監視スレッド
```

### TailReader の仕組み

各JSONLファイルの「最後に読んだバイト位置」をHashMapで管理。`read_new()` で前回位置から末尾まで読み、`Vec<RawEntry>` を返す。常に差分（新規追記行）のみを処理する。

### JSOALエントリの種類と処理

| entryのtype | subtype | 処理 |
|---|---|---|
| `assistant` | — | tool_use を抽出 → `classify_entry()` で WorkKind 判定 |
| `user` | — | 人間のプロンプト → `Thinking` に遷移 / tool_result → WorkKind継続 |
| `system` | `turn_duration` | ターン終了 → `Idle` に遷移、active_skill/todos をクリア |
| `assistant` | isSidechain=true | サブエージェントのJSONL（agent_id で振り分け） |

### ツール名 → WorkKind のマッピング（classify.rs）

```
Read, NotebookRead            → Reading
Edit, Write, NotebookEdit     → Editing
Bash, PowerShell              → Running（detail = descriptionフィールド）
Grep, Glob, ToolSearch        → Searching
WebFetch, WebSearch           → WebExploring
Agent, Task                   → Delegating
AskUserQuestion               → AwaitingUser
TodoWrite                     → WorkKind変更なし（todosのみ更新）
Skill                         → WorkKind変更なし（active_skillのみ更新）
その他                         → Reviewing
```

### セッションステータス遷移

```
最終イベントから経過時間で自動遷移（5秒ごとのタイマーで再計算）

  0 〜 5分    → Active
  5分 〜 15分  → Idle
  15分以降    → Ended（タウンから除去）

※ JSONL の turn_duration エントリが来たら即座に Idle に遷移
  （異常終了時のフォールバックとして時間ベースも維持）
```

### 表示フィルタリング

`sdk-cli`（SDK/evalによる非インタラクティブ起動）は表示しない。`entrypoint` フィールドが `"sdk-cli"` のセッションを除外。`cli`（インタラクティブ）と `entrypoint` 不明（旧バージョン）は表示する。

### サブエージェントの紐付けロジック

```
メインセッション側：
  Agent/Task ツール呼び出し → subagents[] に agent_id="" で仮登録

サブエージェントJSONL側：
  1. agent_id が一致するエントリを検索
  2. なければ agent_id="" の最新エントリに紐付け
  3. それもなければ新規作成
```

### 設定ファイル監視（watcher/config.rs）

- `~/.claude/settings.json`
- `{project}/.claude/settings.json`（プロジェクトごと）
- `{project}/.claude.local/settings.json`（ローカルスコープ）

を監視し、変更時に `"state://config/updated"` を emit する。

**エコーバック抑制**: GUI自身が書き込んだファイルに反応しないよう、`mark_written()` で記録し `is_self_write()` で1.5秒間スキップする。

---

## 6. フロントエンド設計

### Store 構成（Zustand）

```
worldStore
  sessions: Session[]          ← "state://sessions/updated" イベントで更新
  loaded: boolean
  start()                      ← 起動時に get_initial_state() を呼び、listenを開始

hookStore
  flashes: Record<key, HookFlash>  ← "state://lifecycle/fired" イベントで更新
  start()                      ← ライフサイクルイベントのリッスンを開始
  ※ key = agent_id ?? session_id

configStore
  hooks: HooksMap              ← get_hooks() で取得
  agents: AgentDef[]
  skills: SkillDef[]
  loadAll()
  watch()                      ← "state://config/updated" で再フェッチ

effectiveHooksStore
  byProject: Record<project, EffectiveHooks>
  ensure(projects[])           ← 未取得プロジェクトのみ get_effective_hooks() を呼ぶ
  refresh()                    ← 全プロジェクトを再フェッチ

metricsStore / scopedMetricsStore
  metrics: AgentMetrics[]
  ensureLoaded()               ← 初回のみ get_metrics() を呼ぶ（重い処理のため遅延ロード）

characterStore
  外見（色・目）のカスタマイズ設定。localStorageで永続化

uiPrefsStore
  hookView: boolean            ← Hookレール/ビームの表示ON/OFF
```

### IPC 層（src/ipc/）

```typescript
// commands.ts：invoke() のタイプセーフなラッパー
api.getInitialState()           → get_initial_state
api.getHooks(project?)          → get_hooks
api.getEffectiveHooks(project)  → get_effective_hooks
api.updateHooks(hooks, project?)→ update_hooks
api.saveAgent(agent, create)    → save_agent
api.deleteAgent(name)           → delete_agent
api.toggleSkill(name, disable)  → toggle_skill
api.saveSkill(skill, create)    → save_skill
api.getMetrics(project?)        → get_metrics
api.getSessionTimeline(sid,aid) → get_session_timeline
api.focusTerminal(sid, project) → focus_terminal

// events.ts：emit() のリスナーラッパー
onSessionsUpdated(cb)           ← "state://sessions/updated"
onLifecycleFired(cb)            ← "state://lifecycle/fired"
onConfigUpdated(cb)             ← "state://config/updated"
```

### PixiJS 描画エンジン（src/office/engine/）

```
Stage
  PixiJS Application を初期化
  ドラッグ/ピンチでパン・ズーム（zoomMath.ts で制約）
  時刻 → 時間帯テーマ切替（timeOfDay.ts: day/evening/night）
  onTap / onHover コールバックをOfficeViewに公開

WorldRenderer（メインの描画制御）
  scene: Container
  ├── bg: TilingSprite（地面テクスチャ）
  ├── contentLayer（家具・キャラ、zIndexでソート）
  ├── signLayer（看板・アイコン・HookRail）
  └── calloutLayer（吹き出し・テザー線・HookBeam）

  sync(sessions)           ← セッション追加/削除/更新の差分処理
  update(t)                ← 毎フレーム：キャラ移動・callout配置・beam更新
  applyHooks(flashes)      ← hook発火の可視化
  applyEffectiveHooks(byProject) ← 各部屋のrailに登録hookを反映

OrchestratorSprite / EmployeeSprite
  apply(session/run)       ← ステータス変更の反映
  triggerHook(label, t)    ← バッジ点灯
  update(t)                ← アニメーション更新

Callout（吹き出し）
  WorkKind / detail / active_skill / todos を表示
  calloutLayout.ts でオーバーラップ回避（最適配置計算）

HookRail（部屋の奥壁に設置されるレール）
  各Hookイベント（PreToolUse/PostToolUse等）のソケットを表示
  クリックで HookDetailDialog を開く

HookBeam（発火アニメーション）
  Pre → キャラからソケットへ往路（Pending状態）
  Post → Pendingを解決して復路アニメーション（成功/失敗で色が変わる）
  その他 → 往復アニメーション
```

### アイソメトリック座標系（iso.ts）

```
セル座標 (col, row) → スクリーン座標 (x, y)
  x = (col - row) * TILE_W / 2
  y = (col + row) * TILE_H / 2

TILE_W = 128px, TILE_H = 64px（2:1比率の標準アイソメトリック）
深度ソート：zIndex = world.y（画面下のスプライトほど手前）
```

### 部屋レイアウト（roomLayout.ts）

- `planRoom(workingKeys[], idleCount)`: 作業中エージェント数に応じて部屋サイズを決定し、各エージェントに固定デスク位置（cell + facing）を割り当てる
- `planTown(rooms[])`: 複数部屋を横並びに配置
- `Wanderer.ts`: 非アクティブなキャラを待機エリア内でランダム散歩させる（滑らかな速度変化で自然な動きを実現）

### 多言語対応（i18n）

- 対応言語: `ja / en / de / es / fr / ko / zh`
- `src/i18n/messages/ja.ts` が型定義のソース（他言語は同型を満たすことをTSで強制）
- 翻訳漏れ・キー不一致がコンパイルエラーになる
- `t("キー", { 変数 })` 形式で呼び出し
- ブラウザの `navigator.language` から自動選択、Settings画面で手動変更可

---

## 7. 各機能モジュール仕様

### 7-1. タウン（Office）画面

**目的**: Claude Codeセッションの活動をリアルタイムにアイソメトリックタウンで可視化する

| 操作 | 動作 |
|---|---|
| ドラッグ | パン移動 |
| ピンチ / Ctrl+スクロール | ズーム |
| キャラクタークリック | CharacterLogDialog を表示（活動ログ） |
| HookRailクリック | HookDetailDialog を表示（登録Hook一覧） |
| 看板のメニューアイコン（☰）クリック | RoomMenuOverlay を表示（Metrics・Hooks等へのショートカット） |
| 看板のターミナルアイコン（>_）クリック | そのセッションのホストターミナルをフォーカス |
| タブ切替 | PixiJSを破棄せずパン/ズーム位置を保持して復元 |

**空状態**: 表示対象セッションが0件のとき「タウンは静かです」メッセージを表示

### 7-2. Metricsダッシュボード

**目的**: エージェントごとの稼働実績を数値で把握する

| 項目 | 内容 |
|---|---|
| 集計ウィンドウ | 今日 / 直近7日 / 直近30日 |
| 表示カラム | 稼働シェア / 稼働時間 / 呼び出し回数 / ツール実行数 / 失敗率 / トークン(out) |
| ソート | 稼働シェア降順（固定） |
| 集計対象 | `~/.claude/projects/` 以下の全JSONL（重いため初回のみフルスキャン） |
| 更新 | 「更新」ボタンで手動再集計 |

`project` 引数を渡すとプロジェクトスコープに絞り込み可能（RoomMenuからも呼び出される）。

### 7-3. Agents管理画面

**目的**: `~/.claude/agents/` のエージェント定義を閲覧・作成・編集・削除する

| 操作 | 動作 |
|---|---|
| 一覧表示 | User / Project / Plugin の全エージェントを表示 |
| クリック | AgentDetailを展開（説明・ツール・モデル・稼働統計・役割定義） |
| 「雇用」ボタン | AgentEditorダイアログで新規作成 |
| 「編集」ボタン | AgentEditorで既存定義を編集 |
| 「解雇」ボタン | 定義ファイルを削除（確認ダイアログあり） |
| Plugin由来 | 読み取り専用（編集・削除不可） |

**AgentDef の Markdown フォーマット**:

```markdown
---
name: react-reviewer
description: Reactコンポーネントのレビュー担当
tools: Read,Grep,Glob
model: claude-haiku-4-5-20251001
color: "#4a90e2"
---
（役割定義プロンプト本文）
```

### 7-4. Hooks管理画面

**目的**: `settings.json` の `hooks` セクションを閲覧・追加・削除する

**スコープ選択**: User（`~/.claude/`） / Project（`<project>/.claude/`）を切替

| 操作 | 動作 |
|---|---|
| 一覧表示 | イベント名ごとにグループ化して表示 |
| 追加 | イベント / matcher / commandを入力して保存 |
| 削除 | 対象hookを選択して削除（確認ダイアログあり） |
| 保存 | update_hooks()でRust側に書き込み（既存の他キーは保全） |

**有効hook表示（Effective Hooks）**: User + Project + Local + Plugin の hooks を統合した実効設定を表示。各hookにスコープバッジを付与。

**イベント種別**:
```
SessionStart / UserPromptSubmit / PreToolUse / PostToolUse / Stop / SubagentStop
```

### 7-5. Skills管理画面

**目的**: `~/.claude/skills/` のスキルを閲覧・有効化・無効化・作成する

| 操作 | 動作 |
|---|---|
| 一覧表示 | User / Project / Plugin の全スキルを表示 |
| 有効/無効切替 | `SKILL.md` → `SKILL.md.disabled` のリネームで無効化（本文を保持） |
| 「新規Skill」 | SkillEditorダイアログで新規作成 |
| Plugin由来 | 読み取り専用（無効化は可） |

### 7-6. Settings画面

**目的**: 表示設定とキャラクター外見のカスタマイズ

| 設定項目 | 内容 |
|---|---|
| Hook可視化 | タウンのHookRail・発火ビームの表示ON/OFF |
| ツール名表示 | ログダイアログでツール名を表示するかどうか |
| 表示言語 | ja/en/de/es/fr/ko/zh から選択 |
| キャラクターエディタ | Orchestrator・各Agentの本体色・目色をピクセルアート上でペイント |

### 7-7. ターミナルフォーカス機能

**目的**: タウンのセッションをクリックして元のターミナルウィンドウを前面に出す

**フロー（Windows実装）**:

```
1. session_id → ~/.claude/sessions/{pid}.json をスキャン → Claude PID 取得
2. sysinfo でプロセスツリーを構築 → ホストターミナルを特定
3a. VS Code → code.cmd -r <project> を実行
3b. その他（Windows Terminal / PowerShell / Cmd / Git Bash）
    → EnumWindows + SetForegroundWindow でウィンドウをフォーカス
```

**対応ターミナル（Windows）**: VS Code / Windows Terminal / PowerShell / Command Prompt / Git Bash

---

## 8. Windows固有実装メモ

> `feat/windows-port` ブランチで実施した変更の記録。

### 8-1. ターミナルフォーカス（terminal_cmd.rs）

**VS Code（最優先）**

`code.cmd` を `PATH` → `%LOCALAPPDATA%\Programs\Microsoft VS Code\bin\` → `%ProgramFiles%\Microsoft VS Code\bin\` の順に探索し、`code.cmd -r <project_folder>` で VS Code ウィンドウをフォーカスする。

**その他ターミナル（Windows APIベスト・エフォート）**

`EnumWindows` でプロセスPIDに対応するウィンドウを列挙し、`IsWindowVisible` で可視ウィンドウのみフィルタ、`SetForegroundWindow` でフォーカスする。

**制限**: classic `conhost.exe` ホスト（スタンドアロンのPowerShell/Cmd/Git Bash）ではウィンドウがshellの子プロセス `conhost.exe` に帰属するため、shell PIDではEnumWindowsがヒットせず `window_focused=false` になる場合がある。Windows 11 + Windows Terminalがデフォルトコンソールホストの場合は問題なし。

**使用するWindowsクレート**:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
] }
```

### 8-2. ターミナル検出（terminal/mod.rs）

プロセスツリーを `sysinfo` で構築し、Claude PIDの祖先を遡ってホストターミナルを特定する。

```rust
pub enum TerminalKind {
    VsCode,           // code.exe
    WindowsTerminal,  // WindowsTerminal.exe
    PowerShell,       // pwsh.exe / powershell.exe
    Cmd,              // cmd.exe
    GitBash,          // bash.exe
    Unknown,
}
```

### 8-3. Cargo.toml の条件依存

```toml
# Windows専用依存はplatformフィルタで分離
[target.'cfg(target_os = "windows")'.dependencies]
windows = { ... }
```

### 8-4. パス区切り文字

Rustの `std::path::Path` / `PathBuf` を使えばOS差異は自動吸収される。`glob` クレートはバックスラッシュとスラッシュの両方を受け付けるため、`display()` を使った glob パターン文字列も実用上は動作する。

### 8-5. リリースワークフローの現状と課題

現状のGitHub Actions（`.github/workflows/release.yml`）はmacOS専用のみ。Windowsビルドを追加する場合に必要な変更:

- `windows-latest` ランナーでのビルドジョブを追加
- コード署名証明書の設定（Windowsインストーラーのセキュリティ警告を回避）
- `.exe` / `.msi` インストーラーの配布方法を決定

### 8-6. アイコン

`src-tauri/icons/icon.ico` を追加し、`tauri.conf.json` の `bundle.icon` に登録済み。macOSの `.icns` と共存する形で管理する。

---

## 9. 新規アプリへの転用ガイド

### 9-1. そのまま流用できる部分

| モジュール | 転用価値 | 備考 |
|---|---|---|
| Tauriセットアップ全体 | ★★★ | `lib.rs` / `build.rs` / `tauri.conf.json` / `capabilities/` の構成がそのまま使える |
| `config_io/safe_write.rs` | ★★★ | バックアップ付き安全書き込みの汎用実装 |
| `model/` のts-rs連携 | ★★★ | Rust型 → TypeScript型の自動生成パターン |
| `watcher/projects.rs` | ★★ | JSONL tail監視の実装パターン（対象ファイルを変えれば別アプリにも使える） |
| `ipc/commands.ts` の構造 | ★★★ | `invoke()` をタイプセーフにラップするパターン |
| Zustand Store設計 | ★★ | Push型（listen）とPull型（invoke）を分離するパターン |
| i18nシステム | ★★★ | jaを型定義のソースにする多言語対応は再利用しやすい |
| `terminal/mod.rs` | ★★ | Windowsのターミナル検出ロジック |

### 9-2. アプリ固有のため入れ替える部分

| モジュール | 理由 |
|---|---|
| `pipeline/` (session_tracker, classify) | Claude Code JSONL専用のパース・分類ロジック |
| `office/engine/` (PixiJS描画エンジン) | タウン表示専用。別のUIなら丸ごと置き換える |
| `metrics/` | Claude Codeの稼働統計計算。別データソースなら再設計 |
| `model/session.rs` / `activity.rs` | Claude Code概念（Session/SubAgent/WorkKind）が前提 |

### 9-3. 新規アプリ構築の推奨手順

```
1. このリポジトリをテンプレートとしてコピー
   （git clone → 新リポジトリとして初期化）

2. src-tauri/tauri.conf.json を変更
   - productName / identifier / version を新アプリ用に設定
   - bundle.icon を新しいアイコンに差し替え

3. モデル定義を書き直す（src-tauri/src/model/）
   - アプリのドメインに合わせた Rust 型を定義
   - #[derive(TS)] と #[ts(export)] を付ければ自動的にTypeScript型が生成される

4. バックエンドロジックを実装（src-tauri/src/）
   - watcher/ や pipeline/ をアプリのデータソースに合わせて改修
   - commands/ に新しい Tauri コマンドを追加
   - lib.rs の invoke_handler に登録

5. フロントエンドを作り直す（src/）
   - src/bindings/ は Rust ビルド時に自動生成されるので手を加えない
   - src/ipc/commands.ts に新コマンドのラッパーを追加
   - Zustand Store を新ドメインに合わせて設計
   - UI コンポーネントを実装（PixiJSは不要ならpixi.jsを削除）

6. i18n を引き継ぐ場合
   - src/i18n/messages/ja.ts をソースとして書き直す
   - 他言語は型エラーで翻訳漏れが検出される
```

### 9-4. アーキテクチャ上の注意点

**AppStateはグローバルシングルトン**

Tauriの `manage()` で登録したものが全コマンドから `State<'_, AppState>` で参照できる。`Mutex<World>` でロックするため、長時間ロックを持つコマンドは非同期にする。

**エコーバック抑制は必須**

GUIが設定ファイルを書き込むと、ファイル監視が反応して再読み込みしてしまう。`mark_written()` / `is_self_write()` で1.5秒間スキップする実装を維持すること。

**型の単一の真実（Single Source of Truth）**

`Rust モデル → ts-rs → TypeScript 型` の流れを維持する限り、IPC境界での型ズレは発生しない。TypeScript側で独自型を定義してIPCと手動で合わせる実装は避ける。
