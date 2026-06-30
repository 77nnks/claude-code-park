# Claude Code Park Windows 移植 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** macOS 固有のコード（AppleScript / `ps` / `.app` バンドルパス）を Windows ネイティブな実装（`sysinfo` / Win32 API）に置き換え、Claude Code Park を Windows で動作させる。

**Architecture:** `terminal/mod.rs` でプロセスツリー探索ロジックを `sysinfo` ベースに書き直し、`terminal_cmd.rs` でターミナルフォーカスを VS Code CLI（`code -r`）または `SetForegroundWindow` で実装する。フロントエンド・ファイル監視・JSONL 処理は変更しない。

**Tech Stack:** Tauri v2, Rust, sysinfo 0.32, windows-rs 0.58, React (変更なし)

## Global Constraints

- Rust edition: 2021、rust-version: 1.77 以上
- macOS 互換性は不要。Windows ネイティブとして実装する
- `model/`, `config_io/`, `jsonl/`, `metrics/`, `pipeline/`, `watcher/`, `paths.rs`, フロントエンド (`src/`) は変更しない
- TDD: テストを先に書き、失敗を確認してから実装する
- テスト実行コマンド: `cargo test --manifest-path src-tauri/Cargo.toml`
- ビルド確認コマンド: `cargo check --manifest-path src-tauri/Cargo.toml`

---

## File Map

| ファイル | 役割 | 変更種別 |
|---------|------|---------|
| `src-tauri/Cargo.toml` | 依存クレート定義 | Modify |
| `src-tauri/src/terminal/mod.rs` | TerminalKind・ProcInfo・classify_comm・find_host_terminal・parse_session_entry | Full rewrite |
| `src-tauri/src/commands/terminal_cmd.rs` | focus_terminal Tauri コマンド・vscode_cli_candidates・focus_window_by_pid | Full rewrite |
| `src-tauri/src/model/terminal.rs` | FocusResult 型（コメント更新のみ） | Minor edit |
| `README.md` | Requirements / Build セクション | Modify |

---

## Task 1: Cargo.toml に依存クレートを追加する

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `sysinfo = "0.32"`、`windows = "0.58"` が利用可能

- [ ] **Step 1: `[dependencies]` セクションに sysinfo と windows を追記する**

`src-tauri/Cargo.toml` の `[dependencies]` セクション末尾に追加する:

```toml
sysinfo = "0.32"
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
] }
```

`Cargo.toml` の当該箇所（変更後の全 `[dependencies]` セクション）:

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
ts-rs = "10"
notify = "7"
notify-debouncer-full = "0.4"
serde_yaml = "0.9"
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
glob = "0.3"
sysinfo = "0.32"
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
] }
```

- [ ] **Step 2: cargo check でコンパイルが通ることを確認する**

```
cargo check --manifest-path src-tauri/Cargo.toml
```

期待: エラーなし（警告は無視して OK）

- [ ] **Step 3: コミットする**

```
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add sysinfo and windows crates for Windows port"
```

---

## Task 2: `terminal/mod.rs` を Windows 向けに書き直す

**Files:**
- Rewrite: `src-tauri/src/terminal/mod.rs`

**Interfaces:**
- Consumes: `sysinfo::System`（Task 1 で追加）、`serde`, `serde_json`
- Produces:
  - `pub enum TerminalKind` — VsCode / WindowsTerminal / PowerShell / Cmd / GitBash / Unknown
  - `pub struct ProcInfo { pub pid: u32, pub parent_pid: Option<u32>, pub exe: String }`
  - `pub struct HostTerminal { pub pid: u32, pub kind: TerminalKind }`
  - `pub fn classify_comm(exe_path: &str) -> TerminalKind`
  - `pub fn collect_procs(sys: &sysinfo::System) -> Vec<ProcInfo>`
  - `pub fn find_host_terminal(start_pid: u32, procs: &[ProcInfo]) -> Option<HostTerminal>`
  - `pub fn parse_session_entry(json: &str) -> Option<(u32, String)>`

- [ ] **Step 1: テストを書く（まだ実装しない）**

`src-tauri/src/terminal/mod.rs` をテストだけ含む状態に書き換える:

```rust
// tests only — impl below will make these pass

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent_pid: Option<u32>, exe: &str) -> ProcInfo {
        ProcInfo { pid, parent_pid, exe: exe.to_string() }
    }

    // --- classify_comm ---

    #[test]
    fn classify_code_exe_is_vscode() {
        let path = r"C:\Users\user\AppData\Local\Programs\Microsoft VS Code\Code.exe";
        assert_eq!(classify_comm(path), TerminalKind::VsCode);
    }

    #[test]
    fn classify_code_insiders_is_vscode() {
        assert_eq!(classify_comm(r"C:\Program Files\Code - Insiders.exe"), TerminalKind::VsCode);
    }

    #[test]
    fn classify_code_helper_is_vscode() {
        assert_eq!(classify_comm(r"C:\some\path\Code - Helper.exe"), TerminalKind::VsCode);
    }

    #[test]
    fn classify_windows_terminal() {
        assert_eq!(
            classify_comm(r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal\WindowsTerminal.exe"),
            TerminalKind::WindowsTerminal,
        );
    }

    #[test]
    fn classify_pwsh_is_powershell() {
        assert_eq!(
            classify_comm(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            TerminalKind::PowerShell,
        );
    }

    #[test]
    fn classify_powershell_is_powershell() {
        assert_eq!(
            classify_comm(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            TerminalKind::PowerShell,
        );
    }

    #[test]
    fn classify_cmd() {
        assert_eq!(classify_comm(r"C:\Windows\System32\cmd.exe"), TerminalKind::Cmd);
    }

    #[test]
    fn classify_bash_is_git_bash() {
        assert_eq!(
            classify_comm(r"C:\Program Files\Git\usr\bin\bash.exe"),
            TerminalKind::GitBash,
        );
    }

    #[test]
    fn classify_unknown_for_unrecognized() {
        assert_eq!(classify_comm(r"C:\Windows\System32\svchost.exe"), TerminalKind::Unknown);
        assert_eq!(classify_comm("node.exe"), TerminalKind::Unknown);
    }

    // --- find_host_terminal ---

    #[test]
    fn find_host_prefers_vscode_over_powershell_in_chain() {
        // VS Code 統合ターミナルのチェーン: claude (node) -> pwsh.exe -> Code.exe
        let procs = vec![
            p(100, Some(200), "node.exe"),
            p(200, Some(300), r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            p(300, Some(1),   r"C:\Users\u\AppData\Local\Programs\Microsoft VS Code\Code.exe"),
        ];
        let host = find_host_terminal(100, &procs).unwrap();
        assert_eq!(host.pid, 300);
        assert_eq!(host.kind, TerminalKind::VsCode);
    }

    #[test]
    fn find_host_returns_powershell_when_no_vscode() {
        // スタンドアロン PowerShell: claude -> powershell.exe
        let procs = vec![
            p(100, Some(200), "node.exe"),
            p(200, Some(1),   r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
        ];
        let host = find_host_terminal(100, &procs).unwrap();
        assert_eq!(host.pid, 200);
        assert_eq!(host.kind, TerminalKind::PowerShell);
    }

    #[test]
    fn find_host_returns_none_when_no_known_terminal() {
        let procs = vec![p(100, Some(1), "node.exe")];
        assert!(find_host_terminal(100, &procs).is_none());
    }

    #[test]
    fn find_host_stops_at_pid_4_or_below() {
        // pid=4 は Windows システムプロセス。ループを超えないこと。
        let procs = vec![
            p(100, Some(4), "node.exe"),
            p(4,   None,    r"C:\Windows\System32\Code.exe"), // 到達すべきでない
        ];
        assert!(find_host_terminal(100, &procs).is_none());
    }

    // --- parse_session_entry ---

    #[test]
    fn parse_session_entry_extracts_pid_and_sid() {
        let json = r#"{"pid":90684,"sessionId":"1aa1f939-f6a0-4f8c","cwd":"/x","status":"busy"}"#;
        let (pid, sid) = parse_session_entry(json).unwrap();
        assert_eq!(pid, 90684);
        assert_eq!(sid, "1aa1f939-f6a0-4f8c");
    }

    #[test]
    fn parse_session_entry_rejects_missing_pid() {
        assert!(parse_session_entry(r#"{"sessionId":"x"}"#).is_none());
    }

    #[test]
    fn parse_session_entry_rejects_garbage() {
        assert!(parse_session_entry("not json").is_none());
    }
}
```

- [ ] **Step 2: テストが失敗することを確認する**

```
cargo test --manifest-path src-tauri/Cargo.toml terminal::
```

期待: コンパイルエラー（型が未定義）

- [ ] **Step 3: 実装を書く**

`src-tauri/src/terminal/mod.rs` を以下の全内容に置き換える:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Windows で動作するターミナルアプリの種別。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalKind {
    VsCode,          // Code.exe, Code - Insiders.exe, Code Helper.exe など
    WindowsTerminal, // WindowsTerminal.exe
    PowerShell,      // pwsh.exe, powershell.exe
    Cmd,             // cmd.exe
    GitBash,         // bash.exe (Git for Windows)
    Unknown,
}

/// sysinfo から取り出した 1 プロセスの情報。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub exe: String,
}

/// 特定されたホストターミナル。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTerminal {
    pub pid: u32,
    pub kind: TerminalKind,
}

/// TerminalKind の優先度。チェーン全体を走査して最優先のものを選ぶために使う。
/// （VS Code 統合ターミナルでは claude→pwsh→Code.exe となるため、
///   最初に止まると PowerShell が選ばれてしまう。優先度で解決する。）
fn priority(kind: &TerminalKind) -> u8 {
    match kind {
        TerminalKind::VsCode          => 10,
        TerminalKind::WindowsTerminal => 9,
        TerminalKind::PowerShell      => 5,
        TerminalKind::Cmd             => 4,
        TerminalKind::GitBash         => 4,
        TerminalKind::Unknown         => 0,
    }
}

/// exe のフルパスから TerminalKind を判定する。ファイル名（basename）を小文字化して比較する。
pub fn classify_comm(exe_path: &str) -> TerminalKind {
    let name = Path::new(exe_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if name.starts_with("code") && name.ends_with(".exe") {
        // Code.exe / Code - Insiders.exe / Code Helper.exe / Code - OSS.exe など
        TerminalKind::VsCode
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

/// `sysinfo::System` のスナップショットから `Vec<ProcInfo>` を構築する。
/// `terminal_cmd.rs` から呼ばれる。テスト時はこの関数を経由せず直接 `Vec<ProcInfo>` を構築できる。
pub fn collect_procs(sys: &sysinfo::System) -> Vec<ProcInfo> {
    sys.processes()
        .values()
        .map(|p| ProcInfo {
            pid: p.pid().as_u32(),
            parent_pid: p.parent().map(|pp| pp.as_u32()),
            exe: p.exe()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default(),
        })
        .collect()
}

/// `start_pid` からプロセスチェーンを親方向に辿り、最優先のターミナルを返す。
/// 上限 64 ステップ。pid <= 4 は Windows システムプロセスとして停止する。
pub fn find_host_terminal(start_pid: u32, procs: &[ProcInfo]) -> Option<HostTerminal> {
    let by_pid = |pid: u32| procs.iter().find(|p| p.pid == pid);
    let mut candidates: Vec<HostTerminal> = Vec::new();
    let mut cur = start_pid;

    for _ in 0..64 {
        let proc = by_pid(cur)?;
        let kind = classify_comm(&proc.exe);
        if kind != TerminalKind::Unknown {
            candidates.push(HostTerminal { pid: cur, kind });
        }
        match proc.parent_pid {
            Some(ppid) if ppid > 4 => cur = ppid,
            _ => break,
        }
    }

    candidates.into_iter().max_by_key(|h| priority(&h.kind))
}

/// `~/.claude/sessions/<pid>.json` の内容から (pid, sessionId) を取り出す。
/// Claude Code が各実行セッション用に書くファイル。
pub fn parse_session_entry(json: &str) -> Option<(u32, String)> {
    #[derive(Deserialize)]
    struct SessionFile {
        pid: u32,
        #[serde(rename = "sessionId")]
        session_id: String,
    }
    let f: SessionFile = serde_json::from_str(json).ok()?;
    Some((f.pid, f.session_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent_pid: Option<u32>, exe: &str) -> ProcInfo {
        ProcInfo { pid, parent_pid, exe: exe.to_string() }
    }

    #[test]
    fn classify_code_exe_is_vscode() {
        let path = r"C:\Users\user\AppData\Local\Programs\Microsoft VS Code\Code.exe";
        assert_eq!(classify_comm(path), TerminalKind::VsCode);
    }

    #[test]
    fn classify_code_insiders_is_vscode() {
        assert_eq!(classify_comm(r"C:\Program Files\Code - Insiders.exe"), TerminalKind::VsCode);
    }

    #[test]
    fn classify_code_helper_is_vscode() {
        assert_eq!(classify_comm(r"C:\some\path\Code - Helper.exe"), TerminalKind::VsCode);
    }

    #[test]
    fn classify_windows_terminal() {
        assert_eq!(
            classify_comm(r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal\WindowsTerminal.exe"),
            TerminalKind::WindowsTerminal,
        );
    }

    #[test]
    fn classify_pwsh_is_powershell() {
        assert_eq!(
            classify_comm(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            TerminalKind::PowerShell,
        );
    }

    #[test]
    fn classify_powershell_is_powershell() {
        assert_eq!(
            classify_comm(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            TerminalKind::PowerShell,
        );
    }

    #[test]
    fn classify_cmd() {
        assert_eq!(classify_comm(r"C:\Windows\System32\cmd.exe"), TerminalKind::Cmd);
    }

    #[test]
    fn classify_bash_is_git_bash() {
        assert_eq!(
            classify_comm(r"C:\Program Files\Git\usr\bin\bash.exe"),
            TerminalKind::GitBash,
        );
    }

    #[test]
    fn classify_unknown_for_unrecognized() {
        assert_eq!(classify_comm(r"C:\Windows\System32\svchost.exe"), TerminalKind::Unknown);
        assert_eq!(classify_comm("node.exe"), TerminalKind::Unknown);
    }

    #[test]
    fn find_host_prefers_vscode_over_powershell_in_chain() {
        let procs = vec![
            p(100, Some(200), "node.exe"),
            p(200, Some(300), r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            p(300, Some(1),   r"C:\Users\u\AppData\Local\Programs\Microsoft VS Code\Code.exe"),
        ];
        let host = find_host_terminal(100, &procs).unwrap();
        assert_eq!(host.pid, 300);
        assert_eq!(host.kind, TerminalKind::VsCode);
    }

    #[test]
    fn find_host_returns_powershell_when_no_vscode() {
        let procs = vec![
            p(100, Some(200), "node.exe"),
            p(200, Some(1),   r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
        ];
        let host = find_host_terminal(100, &procs).unwrap();
        assert_eq!(host.pid, 200);
        assert_eq!(host.kind, TerminalKind::PowerShell);
    }

    #[test]
    fn find_host_returns_none_when_no_known_terminal() {
        let procs = vec![p(100, Some(1), "node.exe")];
        assert!(find_host_terminal(100, &procs).is_none());
    }

    #[test]
    fn find_host_stops_at_pid_4_or_below() {
        let procs = vec![
            p(100, Some(4), "node.exe"),
            p(4,   None,    r"C:\Windows\System32\Code.exe"),
        ];
        assert!(find_host_terminal(100, &procs).is_none());
    }

    #[test]
    fn parse_session_entry_extracts_pid_and_sid() {
        let json = r#"{"pid":90684,"sessionId":"1aa1f939-f6a0-4f8c","cwd":"/x","status":"busy"}"#;
        let (pid, sid) = parse_session_entry(json).unwrap();
        assert_eq!(pid, 90684);
        assert_eq!(sid, "1aa1f939-f6a0-4f8c");
    }

    #[test]
    fn parse_session_entry_rejects_missing_pid() {
        assert!(parse_session_entry(r#"{"sessionId":"x"}"#).is_none());
    }

    #[test]
    fn parse_session_entry_rejects_garbage() {
        assert!(parse_session_entry("not json").is_none());
    }
}
```

- [ ] **Step 4: テストが通ることを確認する**

```
cargo test --manifest-path src-tauri/Cargo.toml terminal::
```

期待: 全テスト PASS

- [ ] **Step 5: コミットする**

```
git add src-tauri/src/terminal/mod.rs
git commit -m "feat: rewrite terminal/mod.rs for Windows (sysinfo, priority-based host detection)"
```

---

## Task 3: `terminal_cmd.rs` を Windows 向けに書き直す

**Files:**
- Rewrite: `src-tauri/src/commands/terminal_cmd.rs`

**Interfaces:**
- Consumes:
  - `crate::terminal::{collect_procs, find_host_terminal, parse_session_entry, TerminalKind}` (Task 2)
  - `sysinfo::System` (Task 1)
  - `windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow}` (Task 1)
  - `windows::Win32::Foundation::{BOOL, HWND, LPARAM}` (Task 1)
- Produces:
  - `pub async fn focus_terminal(state, session_id, project) -> AppResult<FocusResult>` — Tauri コマンド（シグネチャ変更なし）

- [ ] **Step 1: `vscode_cli_candidates` のテストを書く**

`src-tauri/src/commands/terminal_cmd.rs` を以下のテストだけ含む骨格に書き換える（impl は Step 3 で追加）:

```rust
// skeleton — impl in Step 3

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vscode_cli_candidates_starts_with_code_in_path() {
        let candidates = vscode_cli_candidates();
        // 先頭 2 つは PATH 上の code / code.cmd
        assert_eq!(candidates[0], std::path::PathBuf::from("code"));
        assert_eq!(candidates[1], std::path::PathBuf::from("code.cmd"));
    }

    #[test]
    fn vscode_cli_candidates_includes_localappdata_path() {
        // LOCALAPPDATA が設定されていれば 3 つ目以降にそのパスが含まれる
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let candidates = vscode_cli_candidates();
            let expected = std::path::PathBuf::from(&local)
                .join("Programs\\Microsoft VS Code\\bin\\code.cmd");
            assert!(candidates.contains(&expected), "expected {expected:?} in {candidates:?}");
        }
    }
}
```

- [ ] **Step 2: テストが失敗することを確認する**

```
cargo test --manifest-path src-tauri/Cargo.toml commands::terminal_cmd::
```

期待: コンパイルエラー（`vscode_cli_candidates` 未定義）

- [ ] **Step 3: 実装を書く**

`src-tauri/src/commands/terminal_cmd.rs` を以下の全内容に置き換える:

```rust
use crate::error::{AppError, AppResult};
use crate::model::terminal::FocusResult;
use crate::state::AppState;
use crate::terminal::{collect_procs, find_host_terminal, parse_session_entry, TerminalKind};
use std::path::PathBuf;
use std::process::Command;
use sysinfo::System;
use tauri::State;

/// VS Code CLI 候補パスを優先順に返す。
/// 1. PATH 上の code / code.cmd（VS Code インストーラーが標準で追加）
/// 2. ユーザーインストールパス（%LOCALAPPDATA%\Programs\Microsoft VS Code\bin\code.cmd）
/// 3. システムインストールパス（%ProgramFiles%\Microsoft VS Code\bin\code.cmd）
pub fn vscode_cli_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("code"),
        PathBuf::from("code.cmd"),
    ];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(&local).join("Programs\\Microsoft VS Code\\bin\\code.cmd"),
        );
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidates.push(
            PathBuf::from(&pf).join("Microsoft VS Code\\bin\\code.cmd"),
        );
    }
    candidates
}

/// target_pid に対応する可視ウィンドウを EnumWindows で探し、SetForegroundWindow で前面に出す。
/// 失敗しても panic しない（ベストエフォート）。返値は成功したかどうか。
fn focus_window_by_pid(target_pid: u32) -> bool {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
    };

    struct CallbackState {
        target_pid: u32,
        found: *mut core::ffi::c_void,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let state = &mut *(lparam.0 as *mut CallbackState);
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == state.target_pid && IsWindowVisible(hwnd).as_bool() {
                state.found = hwnd.0;
                return BOOL(0); // 列挙を停止
            }
            BOOL(1) // 継続
        }
    }

    let mut state = CallbackState {
        target_pid,
        found: core::ptr::null_mut(),
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut state as *mut _ as isize),
        );
        if !state.found.is_null() {
            let hwnd = HWND(state.found);
            SetForegroundWindow(hwnd).as_bool()
        } else {
            false
        }
    }
}

/// セッションをクリックしたときにホストターミナルを前面に出す Tauri コマンド。
///
/// フロー:
/// 1. session_id → Claude PID（~/.claude/sessions/<pid>.json をスキャン）
/// 2. sysinfo でプロセスツリーを構築、優先度ベースでホストターミナルを特定
/// 3a. VS Code → code -r <folder>（PATH → 標準インストールパスの順に試す）
/// 3b. その他 → EnumWindows + SetForegroundWindow
#[tauri::command]
pub async fn focus_terminal(
    state: State<'_, AppState>,
    session_id: String,
    project: String,
) -> AppResult<FocusResult> {
    let sessions_dir = state.paths.sessions_dir();
    tauri::async_runtime::spawn_blocking(move || {
        // 1. session_id -> claude PID
        let claude_pid = resolve_claude_pid(&sessions_dir, &session_id).ok_or_else(|| {
            AppError::Other(
                "no running claude process found (the session may have ended)".into(),
            )
        })?;

        // 2. プロセスツリーを sysinfo で取得してホストターミナルを特定
        let sys = System::new_all();
        let procs = collect_procs(&sys);
        let host = find_host_terminal(claude_pid, &procs).ok_or_else(|| {
            AppError::Other("could not identify the host terminal".into())
        })?;

        let app_name = match &host.kind {
            TerminalKind::VsCode          => "Visual Studio Code",
            TerminalKind::WindowsTerminal => "Windows Terminal",
            TerminalKind::PowerShell      => "PowerShell",
            TerminalKind::Cmd             => "Command Prompt",
            TerminalKind::GitBash         => "Git Bash",
            TerminalKind::Unknown         => {
                return Err(AppError::Other("unsupported terminal".into()))
            }
        };

        // 3a. VS Code: code -r <folder> で正確にウィンドウをフォーカス
        if host.kind == TerminalKind::VsCode {
            for candidate in vscode_cli_candidates() {
                let st = Command::new(&candidate).arg("-r").arg(&project).status();
                if matches!(st, Ok(s) if s.success()) {
                    return Ok(FocusResult {
                        app: app_name.to_string(),
                        window_focused: true,
                    });
                }
            }
            // code CLI が見つからない場合はエラー（PATH への追加を促す）
            return Err(AppError::Other(
                "VS Code CLI not found. Run 'Shell Command: Install code command in PATH' \
                 from VS Code's command palette (Ctrl+Shift+P) and restart."
                    .into(),
            ));
        }

        // 3b. その他: EnumWindows + SetForegroundWindow（ベストエフォート）
        let window_focused = focus_window_by_pid(host.pid);
        Ok(FocusResult {
            app: app_name.to_string(),
            window_focused,
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("terminal focus task failed: {e}")))?
}

/// ~/.claude/sessions/*.json をスキャンし、session_id に一致する claude PID を返す。
fn resolve_claude_pid(sessions_dir: &std::path::Path, session_id: &str) -> Option<u32> {
    let pattern = format!("{}/*.json", sessions_dir.display());
    for path in glob::glob(&pattern).into_iter().flatten().flatten() {
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        let Some((pid, sid)) = parse_session_entry(&content) else { continue };
        if sid == session_id {
            return Some(pid);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vscode_cli_candidates_starts_with_code_in_path() {
        let candidates = vscode_cli_candidates();
        assert_eq!(candidates[0], PathBuf::from("code"));
        assert_eq!(candidates[1], PathBuf::from("code.cmd"));
    }

    #[test]
    fn vscode_cli_candidates_includes_localappdata_path() {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let candidates = vscode_cli_candidates();
            let expected = PathBuf::from(&local)
                .join("Programs\\Microsoft VS Code\\bin\\code.cmd");
            assert!(candidates.contains(&expected), "expected {expected:?} in {candidates:?}");
        }
    }
}
```

- [ ] **Step 4: テストが通ることを確認する**

```
cargo test --manifest-path src-tauri/Cargo.toml commands::terminal_cmd::
```

期待: 全テスト PASS

- [ ] **Step 5: ビルド全体が通ることを確認する**

```
cargo check --manifest-path src-tauri/Cargo.toml
```

期待: エラーなし

- [ ] **Step 6: コミットする**

```
git add src-tauri/src/commands/terminal_cmd.rs
git commit -m "feat: rewrite terminal_cmd.rs for Windows (code -r, SetForegroundWindow)"
```

---

## Task 4: model/terminal.rs コメント更新・README 更新

**Files:**
- Minor edit: `src-tauri/src/model/terminal.rs`
- Modify: `README.md`

**Interfaces:**
- Produces: ドキュメントが Windows 環境を反映した状態

- [ ] **Step 1: `model/terminal.rs` の `app` フィールドのコメントを Windows 向けに更新する**

`src-tauri/src/model/terminal.rs` の該当行を変更:

```rust
// 変更前
/// Name of the app brought to the front ("Ghostty" / "Visual Studio Code" / "Terminal"). Always valid since it is only returned on success.
pub app: String,

// 変更後
/// Name of the app brought to the front (e.g. "Visual Studio Code" / "Windows Terminal" / "PowerShell"). Always valid since it is only returned on success.
pub app: String,
```

- [ ] **Step 2: README.md の Requirements・Install セクションを Windows 向けに更新する**

`README.md` の以下のセクションを置き換える:

```markdown
## Requirements

- **Windows 10 / 11** (64-bit)
- [Claude Code](https://docs.claude.com/claude-code) installed, with an existing `~/.claude/`
  (`~/.claude/` は Windows では `C:\Users\<user>\.claude\` を意味します)
- **VS Code terminal focus** を使う場合: VS Code の `code` コマンドを PATH に追加してください。
  VS Code のコマンドパレット (`Ctrl+Shift+P`) → **Shell Command: Install 'code' command in PATH** を実行します。
```

```markdown
## Build from source / Development

Requirements: [Node.js](https://nodejs.org) 18+ / npm, and Rust (stable). Install Rust via [rustup](https://rustup.rs).

> See the [Tauri prerequisites for Windows](https://tauri.app/start/prerequisites/) for full setup (Rust, Visual C++ Build Tools, WebView2 など).

```bash
npm install
npm run app      # = tauri dev (launches the desktop window)
```

Other commands:

```bash
npm run build                                   # type-check + build the frontend
npm run tauri build                             # produce a distributable .exe
cargo test --manifest-path src-tauri/Cargo.toml # Rust tests
```
```

- [ ] **Step 3: 全テストが通ることを確認する**

```
cargo test --manifest-path src-tauri/Cargo.toml
```

期待: 全テスト PASS

- [ ] **Step 4: コミットする**

```
git add src-tauri/src/model/terminal.rs README.md
git commit -m "docs: update model comments and README for Windows"
```

---

## Task 5: 手動動作確認

**Files:** なし（読み取り専用確認タスク）

- [ ] **Step 1: アプリをビルドして起動する**

```
npm run app
```

期待: Tauri ウィンドウが起動し、`~/.claude/` を読み込んでオフィスが表示される

- [ ] **Step 2: VS Code の統合ターミナルから claude を起動する**

VS Code のターミナルで:
```
claude
```

- [ ] **Step 3: セッションフォーカスを確認する**

Claude Code Park のオフィスに新しいセッションが表示されたら、そのキャラクターをクリックする。

期待:
- VS Code ウィンドウが前面に来る
- エラートーストが表示されない

> `code` コマンドが PATH にない場合はエラートーストが表示される。
> その場合: VS Code の `Ctrl+Shift+P` → "Shell Command: Install 'code' command in PATH" を実行してターミナルを再起動する。

- [ ] **Step 4: 最終コミット（動作確認後）**

```
git add -A
git commit -m "chore: Windows port complete — verified terminal focus via VS Code CLI"
```
