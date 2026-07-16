//! Pure HTTP/1.1 request-head parsing and rewriting. No sockets, no
//! processes — split out of `super` purely to stay under the
//! workspace's file-length lint tier (mirrors `lifecycle`'s split).

use std::io::Read;
use std::os::unix::net::UnixStream;

use crate::error::CageError;

use super::ProxyConfig;

/// Hard cap on the request-head size (per the usual server limits).
const MAX_HEAD: usize = 64 * 1024;

/// Whether `line` is a request header the proxy owns (dropped from the
/// client's head and re-emitted with proxy-controlled values). Client
/// credentials are owned only when the proxy injects its own key.
fn is_owned_header(line: &str, inject: bool) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("host:") || lower.starts_with("connection:") {
        return true;
    }
    inject && (lower.starts_with("x-api-key:") || lower.starts_with("authorization:"))
}

/// Rewrite an HTTP/1.1 request head.
///
/// Keeps the request line and all non-owned headers, then sets `Host`,
/// `Connection: close`, and (in key-injection mode) `x-api-key`.
/// Pure; `head` excludes the final blank line.
#[must_use]
pub fn rewrite_head(head: &str, cfg: &ProxyConfig) -> String {
    let inject = cfg.api_key.is_some();
    let mut out = String::new();
    for line in head.split("\r\n") {
        if line.is_empty() || is_owned_header(line, inject) {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str("Host: ");
    out.push_str(&cfg.upstream_host);
    out.push_str("\r\n");
    if let Some(key) = &cfg.api_key {
        out.push_str("x-api-key: ");
        out.push_str(key);
        out.push_str("\r\n");
    }
    out.push_str("Connection: close\r\n\r\n");
    out
}

/// Read from `stream` until the `\r\n\r\n` head terminator; returns
/// `(head_text, leftover_body_bytes)`.
///
/// # Errors
///
/// [`CageError::Protocol`] on EOF before the terminator or an
/// oversized head; [`CageError::Io`] on read failure.
pub(super) fn read_head(stream: &mut UnixStream) -> Result<(String, Vec<u8>), CageError> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(CageError::Protocol("EOF before request head".to_owned()));
        }
        buf.extend_from_slice(chunk.get(..n).unwrap_or_default());
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(pos + 4);
            buf.truncate(pos);
            return Ok((String::from_utf8_lossy(&buf).into_owned(), rest));
        }
        if buf.len() > MAX_HEAD {
            return Err(CageError::Protocol("request head too large".to_owned()));
        }
    }
}
