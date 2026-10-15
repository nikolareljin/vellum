//! System clipboard access without extra crates.
//!
//! Tries the platform clipboard tools in order and falls back to an OSC 52
//! escape sequence, which most modern terminals (kitty, WezTerm, iTerm2,
//! foot, tmux with `set-clipboard on`) turn into a clipboard write, also over SSH.

use std::io::Write;
use std::process::{Command, Stdio};

/// Copy `text` to the system clipboard.  Returns the name of the method used.
pub fn copy(text: &str) -> anyhow::Result<&'static str> {
    for (tool, args) in candidate_tools() {
        if which_exists(tool) && pipe_to(tool, args, text).is_ok() {
            return Ok(tool);
        }
    }
    let mut out = std::io::stdout();
    out.write_all(osc52_sequence(text).as_bytes())?;
    out.flush()?;
    Ok("OSC 52")
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
    fn osc52_frames_payload() {
        assert_eq!(osc52_sequence("a\nb"), "\x1b]52;c;YQpi\x07");
    }
}
