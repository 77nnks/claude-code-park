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
