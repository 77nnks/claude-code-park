# Claude Code Park — Windows 移植設計書

**日付:** 2026-06-29  
**対象バージョン:** 0.1.0 →（Windows 版）  
**ステータス:** 承認済み

---

## 概要

Claude Code Park（Tauri v2 + React + Rust）を macOS 専用から Windows ネイティブへ再実装する。  
macOS との互換性は不要。クロスプラットフォームなクレートは活用しつつ、macOS 固有のコード（AppleScript / `ps` コマンド / `.app` バンドルパス）を Windows の実装で置き換える。

フロントエンド（React / Pixi.js / Zustand）、ファイル監視（`notify`）、JSONL パース、設定 IO、メトリクスは変更しない。

---

## 変更スコープ

### 変更するファイル

| ファイル | 変更内容 |
|---------|---------|
| `src-tauri/src/terminal/mod.rs` | 全面書き直し（Windows 向け） |
| `src-tauri/src/commands/terminal_cmd.rs` | 全面書き直し（Windows 向け） |
| `src-tauri/Cargo.toml` | `sysinfo`・`windows` クレート追加 |
| `README.md` | Requirements を Windows 向けに更新 |

### 変更しないファイル

- `src-tauri/src/paths.rs` — `dirs` クレートが Windows パスを自動解決（`~/.claude` → `C:\Users\<user>\.claude`）
- `src-tauri/src/watcher/` — `notify` クレートはクロスプラットフォーム
- `src-tauri/src/config_io/` — ファイル操作のみ、変更不要
- `src-tauri/src/jsonl/` — クロスプラットフォーム
- `src-tauri/src/metrics/` — クロスプラットフォーム
- `src-tauri/src/pipeline/` — クロスプラットフォーム
- `src/`（React フロントエンド） — 変更なし

---

## 依存クレートの変更

### 追加

```toml
[dependencies]
sysinfo = "0.32"
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
] }
```

**`sysinfo` を選ぶ理由:**  
Tauri 自体が内部で使用しており、プロセスツリー取得（PID・親 PID・exe パス）が 1 つの API で完結する。`wmic` コマンドは非推奨、`tasklist` はパースが不安定、WMI via `windows` クレートは重い。

**`windows` クレートのフィーチャーを最小限にする理由:**  
`windows` クレート全体は巨大。今回使うのは `EnumWindows`・`SetForegroundWindow`・`HWND`・`BOOL` のみなので、必要なフィーチャーだけを明示して追加ビルド時間を最小化する。

### 削除

macOS 専用の `osascript`（AppleScript）呼び出しは依存クレートではなく `std::process::Command` だったため、クレートの削除はない。ただし `glob` クレートは `resolve_claude_pid` 内でセッションファイルの glob パターン走査に使用しており引き続き使用する。

---

## `terminal/mod.rs` — Windows 向け再設計

### TerminalKind（Windows 版）

macOS 固有の `TerminalApp`・`ITerm2`・`Ghostty` を削除し、Windows のターミナルアプリに置き換える。

```rust
pub enum TerminalKind {
    VsCode,           // Code.exe, Code - Insiders.exe, Code Helper.exe
    WindowsTerminal,  // WindowsTerminal.exe
    PowerShell,       // pwsh.exe, powershell.exe
    Cmd,              // cmd.exe
    GitBash,          // bash.exe (Git for Windows)
    Unknown,
}
```

### classify_comm（Windows 版）

`ps` のフルパス文字列（`/Applications/Ghostty.app/...`）の代わりに、exe ファイル名で判定する。

```rust
pub fn classify_comm(exe_path: &str) -> TerminalKind {
    let name = Path::new(exe_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if name.starts_with("code") && name.ends_with(".exe") {
        TerminalKind::VsCode  // Code.exe, Code - Insiders.exe, Code Helper.exe を含む
    } else if name == "windowsterminal.exe" {
        TerminalKind::WindowsTerminal
    } else if name == "pwsh.exe" || name == "powershell.exe" {
        TerminalKind::PowerShell
    } else if name == "cmd.exe" {
        TerminalKind::Cmd
    } else if name == "bash.exe" {
        TerminalKind::GitBash
    } else {
        TerminalKind::Unknown
    }
}
```

### find_host_terminal — 優先度ベースの探索

**macOS との設計変更点:**  
macOS 版は「チェーンを上に辿り、最初に見つかった既知ターミナルで停止」する。  
Windows では VS Code の統合ターミナルのプロセスチェーンが `claude → pwsh.exe → Code.exe` となるため、最初に止まると PowerShell が選ばれてしまう。そこで **チェーン全体を走査し、優先度が最も高いターミナルを選ぶ** 方式に変える。

```
優先度: VsCode(10) > WindowsTerminal(9) > PowerShell(5) > Cmd(4) > GitBash(4) > Unknown(0)
```

```rust
pub fn find_host_terminal(start_pid: u32, sys: &System) -> Option<HostTerminal> {
    let mut candidates: Vec<HostTerminal> = Vec::new();
    let mut cur = Pid::from_u32(start_pid);
    for _ in 0..64 {
        let proc = sys.process(cur)?;
        let exe = proc.exe()?.to_string_lossy();
        let kind = classify_comm(&exe);
        if kind != TerminalKind::Unknown {
            candidates.push(HostTerminal { pid: cur.as_u32(), kind });
        }
        let parent = proc.parent()?;
        if parent.as_u32() <= 4 { break; } // pid 0,1,4 は Windows システムプロセス
        cur = parent;
    }
    candidates.into_iter().max_by_key(|h| priority(&h.kind))
}
```

### 削除するもの

- `ProcRow` 構造体と `parse_ps_rows` 関数 — `sysinfo` が代替
- `parse_session_entry` — 変更なし（ファイルパースなのでプラットフォーム非依存）

---

## `terminal_cmd.rs` — Windows 向けフォーカス実装

### 削除するもの

| 関数・処理 | 理由 |
|-----------|------|
| `build_activate_script` | AppleScript 専用 |
| `build_window_focus_script` | AppleScript 専用 |
| `focus_is_reliable` | TTY ベースの判定（Windows に TTY なし） |
| `vscode_bundle_root` | `.app` バンドル構造（macOS 専用） |
| `vscode_cli_candidates` | `.app` 内パス（macOS 専用） |
| `run_capture("ps", ...)` | `sysinfo` に置き換え |
| TTY 取得（`ps -o tty=`） | Windows に TTY なし |

### フォーカスフロー（Windows 版）

```
focus_terminal(session_id, project)
  │
  ├─ 1. session_id → Claude PID
  │     ~/.claude/sessions/<pid>.json をスキャン（変更なし）
  │
  ├─ 2. sysinfo でプロセスツリー構築・優先度ベースで走査
  │     → HostTerminal { pid, kind } を取得
  │
  ├─ 3a. kind == VsCode
  │       vscode_cli_candidates() を順に試す
  │       └─ code -r <folder> を実行
  │           成功 → FocusResult { window_focused: true }
  │           全失敗 → Err（トースト表示）
  │
  └─ 3b. その他（WindowsTerminal / PowerShell / Cmd / GitBash）
          focus_window_by_pid(host.pid)
          EnumWindows で PID 一致の可視 HWND を探す
          → SetForegroundWindow(hwnd)
          成功 → FocusResult { window_focused: true }
          失敗 → FocusResult { window_focused: false }（ベストエフォート）
```

### VS Code CLI 候補パス（Windows 版）

```rust
fn vscode_cli_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![];
    // 1. PATH 上の code / code.cmd（VS Code インストーラーが標準で追加）
    candidates.push(PathBuf::from("code"));
    candidates.push(PathBuf::from("code.cmd"));
    // 2. ユーザーインストール
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(&local)
            .join("Programs\\Microsoft VS Code\\bin\\code.cmd"));
    }
    // 3. システムインストール
    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(&pf)
            .join("Microsoft VS Code\\bin\\code.cmd"));
    }
    candidates
}
```

### PID によるウィンドウフォーカス（ベストエフォート）

```rust
fn focus_window_by_pid(target_pid: u32) -> bool {
    // EnumWindows コールバックで GetWindowThreadProcessId を使い
    // target_pid に一致する可視ウィンドウの HWND を収集
    // 見つかれば SetForegroundWindow(hwnd) を呼ぶ
    // 失敗してもパニックしない（ベストエフォート）
}
```

### FocusResult の意味（変更なし）

```rust
pub struct FocusResult {
    pub app: String,           // 例: "Visual Studio Code"
    pub window_focused: bool,  // true: 確実にフォーカス成功 / false: 不明
}
```

フロントエンド側の挙動は変更しない。

---

## テスト方針

### ユニットテスト（変更・追加）

| テスト | 内容 |
|--------|------|
| `classify_comm` | `Code.exe` → VsCode、`pwsh.exe` → PowerShell など各パターン |
| `find_host_terminal` | VS Code チェーン（claude→pwsh→Code.exe）で VsCode が優先されること |
| `parse_session_entry` | 変更なし（既存テストをそのまま維持） |
| `vscode_cli_candidates` | パス候補の順序と内容 |

### 手動確認

1. VS Code の統合ターミナルから `claude` を起動
2. Claude Code Park でセッションをクリック
3. VS Code ウィンドウが前面に来ること（`code -r` 経由）

---

## 未対応事項（スコープ外）

- **Windows インストーラー（.msi / .exe）の作成** — 今回は "ビルドできればOK"、配布は対象外
- **GitHub Actions での CI/CD** — 今回は対象外
- **Windows Terminal の詳細フォーカス（タブ指定など）** — `SetForegroundWindow` で十分とみなす
- **Linux 対応** — 今回は対象外
