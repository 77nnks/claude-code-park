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
