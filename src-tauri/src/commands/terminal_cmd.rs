use crate::error::{AppError, AppResult};
use crate::model::terminal::FocusResult;
use crate::state::AppState;
use crate::terminal::{collect_procs, find_host_terminal, parse_session_entry, TerminalKind};
use sysinfo::System;
use tauri::State;

/// Terminal kind -> display name (placeholder for Task 3's Windows focus logic).
pub fn app_name(kind: &TerminalKind) -> Option<&'static str> {
    match kind {
        TerminalKind::VsCode => Some("Visual Studio Code"),
        TerminalKind::WindowsTerminal => Some("Windows Terminal"),
        TerminalKind::PowerShell => Some("PowerShell"),
        TerminalKind::Cmd => Some("Command Prompt"),
        TerminalKind::GitBash => Some("Git Bash"),
        TerminalKind::Unknown => None,
    }
}

/// Brings the session's host terminal to the front (Windows implementation — Task 3).
/// Currently a stub that identifies the terminal kind; window focusing is NYI.
#[tauri::command]
pub async fn focus_terminal(
    state: State<'_, AppState>,
    session_id: String,
    _project: String,
) -> AppResult<FocusResult> {
    let sessions_dir = state.paths.sessions_dir();
    tauri::async_runtime::spawn_blocking(move || {
        // 1. session_id -> running claude PID, via the session file Claude Code writes.
        let claude_pid = resolve_claude_pid(&sessions_dir, &session_id).ok_or_else(|| {
            AppError::Other("no running claude process found (the session may have ended)".into())
        })?;

        // 2. Snapshot all processes and walk the parent chain to find the host terminal.
        let mut sys = System::new_all();
        sys.refresh_all();
        let procs = collect_procs(&sys);
        let host = find_host_terminal(claude_pid, &procs)
            .ok_or_else(|| AppError::Other("could not identify the host terminal".into()))?;

        let app = app_name(&host.kind)
            .ok_or_else(|| AppError::Other("unsupported terminal".into()))?;

        // TODO(Task 3): implement Windows-native terminal focus (SetForegroundWindow / WM_SETFOCUS).
        Err(AppError::Other(format!(
            "focus not yet implemented for {app} on Windows"
        )))
    })
    .await
    .map_err(|e| AppError::Other(format!("terminal focus task failed: {e}")))?
}

/// Scans `~/.claude/sessions/*.json` and returns the claude PID of the file with a matching sessionId.
fn resolve_claude_pid(sessions_dir: &std::path::Path, session_id: &str) -> Option<u32> {
    let pattern = format!("{}/*.json", sessions_dir.display());
    for path in glob::glob(&pattern).into_iter().flatten().flatten() {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some((pid, sid)) = parse_session_entry(&content) else {
            continue;
        };
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
    fn app_name_maps_all_kinds() {
        assert_eq!(app_name(&TerminalKind::VsCode), Some("Visual Studio Code"));
        assert_eq!(app_name(&TerminalKind::WindowsTerminal), Some("Windows Terminal"));
        assert_eq!(app_name(&TerminalKind::PowerShell), Some("PowerShell"));
        assert_eq!(app_name(&TerminalKind::Cmd), Some("Command Prompt"));
        assert_eq!(app_name(&TerminalKind::GitBash), Some("Git Bash"));
        assert_eq!(app_name(&TerminalKind::Unknown), None);
    }
}
