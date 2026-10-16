use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use crate::svg;

/// Load an image from a local file path.
/// SVG/SVGZ files are rasterised via `resvg`; all other formats go through
/// the `image` crate's built-in decoders.
pub fn load_image<P: AsRef<Path>>(path: P) -> Result<DynamicImage> {
    let path_ref = path.as_ref();
    if svg::is_svg_path(&path_ref.to_string_lossy()) {
        let data = std::fs::read(path_ref)?;
        return svg::rasterize(&data);
    }
    let img = image::open(path_ref)?;
    Ok(img)
}

/// Maximum bytes accepted from a remote image response (20 MiB).
const MAX_REMOTE_BYTES: u64 = 20 * 1024 * 1024;
/// Maximum pixel count before decode is rejected (~50 MP, ~200 MiB at 32bpp).
const MAX_REMOTE_PIXELS: u64 = 50_000_000;

/// Returns `true` for `http://` and `https://` URLs, case-insensitively.
/// URI schemes are case-insensitive per RFC 3986 §3.1.
pub fn is_remote_url(s: &str) -> bool {
    s.get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case("http://"))
        || s.get(..8)
            .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
}

/// Sanitise a URL for use in error messages / UI placeholders.
/// Strips query string, fragment, and userinfo (user:pass@) to avoid
/// leaking tokens or credentials.
pub fn safe_error_url(url: &str) -> String {
    // Strip query and fragment.
    let no_query = url
        .split('?')
        .next()
        .and_then(|s| s.split('#').next())
        .unwrap_or(url);
    // Strip userinfo from the authority component (user:pass@host → host).
    if let Some(scheme_end) = no_query.find("://") {
        let after_scheme = scheme_end + 3;
        let rest = &no_query[after_scheme..];
        if let Some(at_pos) = rest.find('@') {
            if !rest[..at_pos].contains('/') {
                return format!("{}{}", &no_query[..after_scheme], &rest[at_pos + 1..]);
            }
        }
    }
    no_query.to_owned()
}

// ── Remote images: off unless the user opts in ───────────────────────────────
//
// Opening a document must not make network requests on its behalf: a remote
// image tells its host who opened the file and when, and its URL can point at
// services inside the user's network.  Fetching is opt-in (`--remote-images`
// or `VELLUM_REMOTE_IMAGES=1`), and an opted-in fetch is https only and never
// connects to a private, loopback or link-local address, redirects included.

static REMOTE_IMAGES: OnceLock<bool> = OnceLock::new();

/// Record the `--remote-images` flag.  Call once, before the first render.
pub fn set_remote_images(enabled: bool) {
    let first = REMOTE_IMAGES.set(enabled).is_ok();
    debug_assert!(
        first,
        "set_remote_images after the setting was already read"
    );
}

/// Whether remote images may be fetched.  `VELLUM_NO_REMOTE_IMAGES` always wins.
pub fn remote_images_enabled() -> bool {
    if std::env::var_os("VELLUM_NO_REMOTE_IMAGES").is_some() {
        return false;
    }
    *REMOTE_IMAGES.get_or_init(|| std::env::var("VELLUM_REMOTE_IMAGES").as_deref() == Ok("1"))
}

/// Host part of a URL, for placeholders: no scheme, userinfo, port or path.
pub fn url_host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if host.starts_with('[') {
        host.split_inclusive(']').next().unwrap_or(host)
    } else {
        host.split(':').next().unwrap_or(host)
    }
}

/// True only for globally routable addresses.  Everything a document could
/// use to reach the user's own machine or network is refused.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (s[0] & 0xffc0) == 0xfe80 // link local fe80::/10
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
                || (s[0] == 0x0064 && s[1] == 0xff9b) // NAT64, embeds IPv4
                || (s[0] == 0x2002) // 6to4, embeds IPv4
                || s[..6] == [0; 6] // IPv4-compatible ::a.b.c.d (deprecated)
                || s[..6] == [0, 0, 0, 0, 0xffff, 0] // IPv4-translated
                || (s[0] == 0x2001 && s[1] == 0) // Teredo 2001::/32
                || s[..4] == [0x0100, 0, 0, 0]) // discard 100::/64
        }
    }
}

fn is_public_v4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    !(v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_unspecified()
        || v4.is_multicast()
        || o[0] == 0 // "this network"
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // CGNAT 100.64/10
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // IETF 192.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // benchmarking 198.18/15
        || o[0] >= 240) // reserved
}

/// DNS resolver that drops non-public addresses.  ureq connects only to what
/// this returns, so a DNS answer cannot steer a fetch (or a redirect) inside.
struct PublicOnly;

impl ureq::Resolver for PublicOnly {
    fn resolve(&self, netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
        let addrs: Vec<SocketAddr> = netloc
            .to_socket_addrs()?
            .filter(|a| is_public_ip(a.ip()))
            .collect();
        if addrs.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "address is not public",
            ));
        }
        Ok(addrs)
    }
}

/// Fetch and decode a remote image.  Requires the user's opt-in and https.
pub fn load_image_url(url: &str) -> Result<DynamicImage> {
    if !remote_images_enabled() {
        anyhow::bail!("remote images are off (use --remote-images)");
    }
    fetch_remote(url)
}

/// The fetch behind `load_image_url`, without the opt-in check.
fn fetch_remote(url: &str) -> Result<DynamicImage> {
    if !url
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
    {
        anyhow::bail!("remote image refused: https only ({})", safe_error_url(url));
    }
    let safe_url = safe_error_url(url);
    let agent = ureq::AgentBuilder::new()
        .https_only(true)
        .redirects(3)
        .resolver(PublicOnly)
        .timeout(Duration::from_secs(10))
        .build();
    let mut buf = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("fetch {safe_url}: {e}"))?
        .into_reader()
        .take(MAX_REMOTE_BYTES + 1)
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_REMOTE_BYTES {
        anyhow::bail!(
            "remote image too large (>{} MiB)",
            MAX_REMOTE_BYTES / 1_048_576
        );
    }
    // SVG has no raster dimensions; handle it before the pixel guard.
    if svg::is_svg_path(&safe_url) {
        return svg::rasterize(&buf);
    }
    // Guard against decompression-bomb images. Fail closed: if imagesize
    // cannot probe the format, reject rather than decode blindly.
    let sz = imagesize::blob_size(&buf)
        .map_err(|_| anyhow::anyhow!("remote image: cannot verify dimensions for {safe_url}"))?;
    let pixels = (sz.width as u64).saturating_mul(sz.height as u64);
    if pixels > MAX_REMOTE_PIXELS {
        anyhow::bail!(
            "remote image dimensions too large ({} × {} px)",
            sz.width,
            sz.height
        );
    }
    Ok(image::load_from_memory(&buf)?)
}

/// In-memory image cache: resolved src path → loaded [`DynamicImage`].
#[derive(Default)]
pub struct ImageCache {
    cache: HashMap<String, DynamicImage>,
}

impl ImageCache {
    pub fn get_or_load(&mut self, src: &str) -> Result<&DynamicImage> {
        if !self.cache.contains_key(src) {
            let img = if is_remote_url(src) {
                load_image_url(src)?
            } else {
                load_image(src)?
            };
            self.cache.insert(src.to_owned(), img);
        }
        Ok(self.cache.get(src).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_global_addresses_are_public() {
        for ip in ["93.184.216.34", "1.1.1.1", "2606:4700::1111"] {
            assert!(is_public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.0.8",
            "198.18.0.1",
            "240.0.0.1",
            "192.0.2.1",
            "::1",
            "::",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "64:ff9b::a00:1",
            "2002:a00:1::",
            "2001:db8::1",
            "::7f00:1",
            "::a00:1",
            "::ffff:0:7f00:1",
            "2001::1",
            "100::1",
        ] {
            assert!(!is_public_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn resolver_refuses_internal_targets() {
        use ureq::Resolver;
        for netloc in [
            "127.0.0.1:443",
            "[::1]:443",
            "169.254.169.254:80",
            "localhost:443",
        ] {
            let err = PublicOnly.resolve(netloc).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied, "{netloc}");
        }
    }

    #[test]
    fn fetch_is_https_only_and_refuses_internal_addresses() {
        let err = fetch_remote("http://example.com/a.png").unwrap_err();
        assert!(err.to_string().contains("https only"), "{err}");
        for url in [
            "https://127.0.0.1/a.png",
            "https://[::1]/a.png",
            "https://localhost/a.png",
            "https://169.254.169.254/latest/meta-data",
        ] {
            let err = fetch_remote(url).unwrap_err().to_string();
            assert!(err.contains("not public"), "{url}: {err}");
        }
    }

    #[test]
    fn url_host_strips_everything_but_the_host() {
        assert_eq!(
            url_host("https://u:p@cdn.example.com:8443/a.png?t=1#f"),
            "cdn.example.com"
        );
        assert_eq!(url_host("https://[2001:db8::1]:443/x"), "[2001:db8::1]");
        assert_eq!(url_host("http://example.com"), "example.com");
    }
}
