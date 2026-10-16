//! System clipboard access.
//!
//! Locally, in order: a clipboard tool (`wl-copy`, `xclip`, `xsel`, `pbcopy`,
//! `clip.exe`; these keep the text after vellum exits), then the native
//! clipboard API, then the OSC 52 escape sequence, which most terminals
//! (kitty, WezTerm, iTerm2, foot, Windows Terminal, xterm with
//! allowWindowOps) turn into a clipboard write.
//!
//! Over SSH only OSC 52 is used: a tool or API on the remote host (X
//! forwarding, a desktop session) would fill the remote clipboard, not the one
//! on the machine the user is typing at.

use std::io::Write;
use std::process::{Command, Stdio};
#[cfg(not(all(unix, not(target_os = "macos"))))]
use std::sync::Mutex;

/// Copy `text` to the system clipboard.  Returns the name of the method used.
pub fn copy(text: &str) -> anyhow::Result<&'static str> {
    let ssh = is_ssh_session();
    if !ssh {
        // A missing tool fails to spawn (NotFound); try the next one.
        for (tool, args) in candidate_tools() {
            if pipe_to(tool, args, &tool_input(tool, text)).is_ok() {
                return Ok(tool);
            }
        }
        if native_copy(text).is_ok() {
            return Ok("system clipboard");
        }
    }
    let in_tmux = std::env::var_os("TMUX").is_some();
    if !ssh && !in_tmux && terminal_ignores_osc52(&|k| std::env::var(k).ok()) {
        // Sending OSC 52 here would report a copy that never happened.
        anyhow::bail!(
            "no clipboard available: install xclip (X11) or wl-clipboard (Wayland); \
             this terminal ignores OSC 52"
        );
    }
    check_osc52_size(text)?;
    let mut out = std::io::stdout();
    out.write_all(osc52_output(text, in_tmux).as_bytes())?;
    out.flush()?;
    Ok(if in_tmux { "OSC 52 (tmux)" } else { "OSC 52" })
}

/// Terminals known to drop OSC 52 clipboard writes without telling anyone.
/// VTE (GNOME Terminal, xfce4-terminal, Tilix, Terminator) sets VTE_VERSION,
/// but a terminal started from one inherits it; any sign of another terminal
/// wins, so kitty or WezTerm launched from GNOME Terminal still get OSC 52.
fn terminal_ignores_osc52(env: &dyn Fn(&str) -> Option<String>) -> bool {
    if env("VTE_VERSION").is_none() {
        return false;
    }
    let other = [
        "KITTY_WINDOW_ID",
        "WEZTERM_PANE",
        "ALACRITTY_WINDOW_ID",
        "KONSOLE_VERSION",
    ]
    .iter()
    .any(|k| env(k).is_some())
        || env("TERM_PROGRAM").is_some_and(|p| !p.is_empty())
        || env("TERM").is_some_and(|t| {
            t == "xterm-kitty" || t == "alacritty" || t.starts_with("foot") || t == "wezterm"
        });
    !other
}

/// One native clipboard handle per session (macOS, Windows).
#[cfg(not(all(unix, not(target_os = "macos"))))]
static NATIVE: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

#[cfg(all(unix, not(target_os = "macos")))]
fn native_copy(text: &str) -> anyhow::Result<()> {
    // X11 and Wayland: the owner must keep serving the text, so hand it to a
    // detached helper that outlives vellum (the way xclip forks).  Its own
    // process group keeps it out of the terminal's SIGHUP and Ctrl+C.
    use std::os::unix::process::CommandExt;
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--clipboard-daemon")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("clipboard helper: no stdin"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("clipboard helper: no stdout"))?;
    stdin.write_all(text.as_bytes())?;
    drop(stdin);
    // The helper writes one byte once it owns the clipboard; EOF means it failed.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut byte = [0u8; 1];
        let _ = tx.send(matches!(stdout.read(&mut byte), Ok(1)));
    });
    let owned = rx.recv_timeout(HELPER_TIMEOUT).unwrap_or(false);
    if !owned {
        // A late helper would take the clipboard after vellum reported failure.
        let _ = child.kill();
    }
    // Reap the helper whenever it exits, so it never lingers as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    if !owned {
        anyhow::bail!("clipboard helper could not take the clipboard");
    }
    Ok(())
}

/// How long the clipboard helper may take to own the clipboard.
#[cfg(all(unix, not(target_os = "macos")))]
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// `vellum --clipboard-daemon`: take text on stdin, own the clipboard until
/// another application replaces it, then exit.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn serve_stdin() -> anyhow::Result<()> {
    use arboard::SetExtLinux;
    use std::io::Read;
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text.clone())?;
    // Tell vellum the copy is in place, then serve it until replaced.
    let mut out = std::io::stdout();
    out.write_all(b"1")?;
    out.flush()?;
    clipboard.set().wait().text(text)?;
    Ok(())
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn serve_stdin() -> anyhow::Result<()> {
    anyhow::bail!("--clipboard-daemon is only used on Linux and BSD")
}

/// macOS and Windows keep clipboard text after the process exits.
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn native_copy(text: &str) -> anyhow::Result<()> {
    let mut guard = NATIVE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new()?);
    }
    guard
        .as_mut()
        .expect("initialised above")
        .set_text(text.to_owned())?;
    Ok(())
}

/// Largest OSC 52 payload (base64 bytes) sent.  xterm drops sequences above
/// about 100 KB and other terminals truncate silently, which would report a
/// copy that did not happen.  This matches the limit common tools use.
const OSC52_MAX_ENCODED: usize = 74_994;

/// Refuse text whose OSC 52 payload a terminal would cut short.
fn check_osc52_size(text: &str) -> anyhow::Result<()> {
    let encoded = text.len().div_ceil(3) * 4;
    if encoded > OSC52_MAX_ENCODED {
        anyhow::bail!(
            "block is too large for OSC 52 ({} KB encoded, terminals accept about {} KB)",
            encoded / 1024,
            OSC52_MAX_ENCODED / 1024
        );
    }
    Ok(())
}

/// How long a clipboard tool may take before the next method is tried.
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn is_ssh_session() -> bool {
    ["SSH_TTY", "SSH_CONNECTION", "SSH_CLIENT"]
        .iter()
        .any(|v| std::env::var_os(v).is_some())
}

/// The bytes to write for an OSC 52 copy.  Inside tmux the plain sequence
/// reaches the outer terminal when `set-clipboard on`; the DCS passthrough
/// form reaches it when `allow-passthrough on`.  Both carry the same text, so
/// sending both works with either setting.
fn osc52_output(text: &str, in_tmux: bool) -> String {
    let seq = osc52_sequence(text);
    if in_tmux {
        let wrapped = seq.replace('\x1b', "\x1b\x1b");
        format!("{seq}\x1bPtmux;{wrapped}\x1b\\")
    } else {
        seq
    }
}

fn candidate_tools() -> Vec<(&'static str, &'static [&'static str])> {
    let mut tools: Vec<(&'static str, &'static [&'static str])> = Vec::new();
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        tools.push(("wl-copy", &[]));
    }
    if std::env::var_os("DISPLAY").is_some() {
        tools.push(("xclip", &["-selection", "clipboard"]));
        tools.push(("xsel", &["--clipboard", "--input"]));
    }
    if cfg!(target_os = "macos") {
        tools.push(("pbcopy", &[]));
    }
    if cfg!(windows) || std::env::var_os("WSL_DISTRO_NAME").is_some() {
        tools.push(("clip.exe", &[]));
    }
    tools
}

/// Bytes a tool expects on stdin.  `clip.exe` reads input without a BOM in the
/// console codepage, which garbles non-ASCII text; UTF-16LE with a BOM is
/// read as Unicode.  The other tools take UTF-8.
fn tool_input(tool: &str, text: &str) -> Vec<u8> {
    if tool == "clip.exe" {
        std::iter::once('\u{FEFF}')
            .chain(text.chars())
            .collect::<String>()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect()
    } else {
        text.as_bytes().to_vec()
    }
}

fn pipe_to(tool: &str, args: &[&str], data: &[u8]) -> anyhow::Result<()> {
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Write on a thread: a tool that is not reading yet would block a large
    // write past the timeout below.  Dropping stdin closes the pipe (EOF).
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("{tool}: no stdin"))?;
    let data = data.to_vec();
    std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    // A tool can block for minutes (xclip connecting to an unreachable X
    // host).  The UI waits on this call, so give up after a short limit.
    let deadline = std::time::Instant::now() + TOOL_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                anyhow::bail!("{tool} exited with {status}");
            }
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("{tool} did not finish within {TOOL_TIMEOUT:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// `ESC ] 52 ; c ; <base64> BEL`: write `text` to the clipboard selection.
fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Standard base64 (RFC 4648) with padding.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn osc52_tmux_adds_passthrough_copy() {
        assert_eq!(osc52_output("a", false), "\x1b]52;c;YQ==\x07");
        assert_eq!(
            osc52_output("a", true),
            "\x1b]52;c;YQ==\x07\x1bPtmux;\x1b\x1b]52;c;YQ==\x07\x1b\\"
        );
    }

    #[cfg(unix)]
    #[test]
    fn stuck_tool_times_out() {
        let start = std::time::Instant::now();
        let err = pipe_to("sleep", &["10"], "x".repeat(1 << 20).as_bytes()).unwrap_err();
        assert!(err.to_string().contains("did not finish"), "{err}");
        assert!(start.elapsed() < TOOL_TIMEOUT + std::time::Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn working_tool_receives_all_text() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let text = "line 1\n  line 2\n".repeat(10_000);
        let script = format!("cat > '{}'", out.display());
        pipe_to("sh", &["-c", &script], text.as_bytes()).unwrap();
        assert_eq!(std::fs::read_to_string(out).unwrap(), text);
    }

    #[test]
    fn clip_exe_gets_utf16le_with_bom() {
        assert_eq!(
            tool_input("clip.exe", "é中"),
            [0xFF, 0xFE, 0xE9, 0x00, 0x2D, 0x4E]
        );
        assert_eq!(tool_input("xclip", "é"), "é".as_bytes());
    }

    #[test]
    fn vte_terminals_get_an_error_not_a_silent_osc52() {
        fn env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
            move |k| {
                vars.iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        }
        let vte = &[("VTE_VERSION", "7600"), ("TERM", "xterm-256color")];
        assert!(terminal_ignores_osc52(&env(vte)));
        assert!(!terminal_ignores_osc52(&env(&[("TERM", "xterm-256color")])));
        // Started from a VTE terminal: the inner terminal identifies itself.
        assert!(!terminal_ignores_osc52(&env(&[
            ("VTE_VERSION", "7600"),
            ("KITTY_WINDOW_ID", "1")
        ])));
        assert!(!terminal_ignores_osc52(&env(&[
            ("VTE_VERSION", "7600"),
            ("TERM_PROGRAM", "WezTerm")
        ])));
        assert!(!terminal_ignores_osc52(&env(&[
            ("VTE_VERSION", "7600"),
            ("TERM", "foot")
        ])));
    }

    #[test]
    fn osc52_size_limit() {
        let max_raw = OSC52_MAX_ENCODED / 4 * 3;
        assert!(check_osc52_size(&"a".repeat(max_raw)).is_ok());
        let err = check_osc52_size(&"a".repeat(max_raw + 1)).unwrap_err();
        assert!(err.to_string().contains("too large for OSC 52"));
    }

    #[test]
    fn osc52_frames_payload() {
        assert_eq!(osc52_sequence("a\nb"), "\x1b]52;c;YQpi\x07");
    }
}
