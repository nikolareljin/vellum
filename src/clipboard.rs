//! System clipboard access without extra crates.
//!
//! Locally, tries the platform clipboard tools and falls back to an OSC 52
//! escape sequence, which most terminals (kitty, WezTerm, iTerm2, foot,
//! Windows Terminal, xterm with allowWindowOps) turn into a clipboard write.
//!
//! Over SSH only OSC 52 is used: a tool on the remote host (xclip with X
//! forwarding, a desktop session's wl-copy) would fill the remote clipboard,
//! not the one on the machine the user is typing at.

use std::io::Write;
use std::process::{Command, Stdio};

/// Copy `text` to the system clipboard.  Returns the name of the method used.
pub fn copy(text: &str) -> anyhow::Result<&'static str> {
    if !is_ssh_session() {
        // A missing tool fails to spawn (NotFound); try the next one.
        for (tool, args) in candidate_tools() {
            if pipe_to(tool, args, text).is_ok() {
                return Ok(tool);
            }
        }
    }
    check_osc52_size(text)?;
    let in_tmux = std::env::var_os("TMUX").is_some();
    let mut out = std::io::stdout();
    out.write_all(osc52_output(text, in_tmux).as_bytes())?;
    out.flush()?;
    Ok(if in_tmux { "OSC 52 (tmux)" } else { "OSC 52" })
}

/// Largest OSC 52 payload (base64 bytes) sent.  xterm drops sequences above
/// about 100 KB and other terminals truncate silently, which would report a
/// copy that did not happen.  This matches the limit common tools use.
pub const OSC52_MAX_ENCODED: usize = 74_994;

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

fn is_ssh_session() -> bool {
    ["SSH_TTY", "SSH_CONNECTION", "SSH_CLIENT"]
        .iter()
        .any(|v| std::env::var_os(v).is_some())
}

/// The bytes to write for an OSC 52 copy.  Inside tmux the plain sequence
/// reaches the outer terminal when `set-clipboard on`; the DCS passthrough
/// form reaches it when `allow-passthrough on`.  Both carry the same text, so
/// sending both works with either setting.
pub fn osc52_output(text: &str, in_tmux: bool) -> String {
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

fn pipe_to(tool: &str, args: &[&str], text: &str) -> anyhow::Result<()> {
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Dropping stdin closes the pipe so the tool sees EOF.
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("{tool}: no stdin"))?
        .write_all(text.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("{tool} exited with {status}");
    }
    Ok(())
}

pub fn which_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `ESC ] 52 ; c ; <base64> BEL`: write `text` to the clipboard selection.
pub fn osc52_sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Standard base64 (RFC 4648) with padding.
pub fn base64(data: &[u8]) -> String {
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
