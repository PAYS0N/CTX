//! Wire protocol for the cage's broker.
//!
//! Client → server: one length-prefixed JSON request. Server →
//! client: a stream of length-prefixed frames terminated by a single
//! `Exit` frame. Every invariant is checked by an explicit length;
//! there are no sentinels, no escape sequences, and no in-band
//! framing-within-framing (the previous Bash transport leaned on a
//! `__CTXRC__N` trailer line and a 0.5s socat timeout — see ADR-028
//! for the failure mode that mandated this redesign).
//!
//! Frame layout (response side):
//!
//! ```text
//! u8 tag | u32 BE length | payload[length]
//! tag 0x00 = OUTPUT (payload = raw bytes; tool's combined stdout+stderr)
//! tag 0x01 = EXIT   (payload = 4-byte BE exit code; ends the stream)
//! ```
//!
//! Request layout (client side):
//!
//! ```text
//! u32 BE length | JSON body { "tool": "...", "args": ["..."] }
//! ```

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::error::CageError;

/// Defensive upper bound on any single frame's payload (1 `MiB`). The
/// broker never emits a chunk larger than its read buffer; this cap
/// keeps a malformed peer from forcing arbitrarily large allocations.
pub const MAX_FRAME_LEN: usize = 1 << 20;

/// Tag byte for an `Output` frame.
const TAG_OUTPUT: u8 = 0;

/// Tag byte for an `Exit` frame.
const TAG_EXIT: u8 = 1;

/// `Exit` payload is always exactly four bytes (a big-endian `u32`).
const EXIT_PAYLOAD_LEN: u32 = 4;

/// A request from the in-cage client to the host-side broker.
///
/// JSON-encoded. Unknown fields on deserialization are ignored, so
/// optional fields can be added without breaking older readers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    /// Tool name to invoke; the broker matches this against its
    /// allowlist and rejects everything else.
    pub tool: String,
    /// Argv to pass to the tool. The broker does no parsing of its
    /// own — it passes these straight to the binary.
    pub args: Vec<String>,
}

/// One frame in the response stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseFrame {
    /// A chunk of the tool's combined stdout/stderr bytes (possibly
    /// empty).
    Output(Vec<u8>),
    /// The tool's exit code; appears exactly once, terminating the
    /// stream.
    Exit(u32),
}

/// Write a request: `u32` length prefix then the JSON body.
///
/// # Errors
///
/// [`CageError::Io`] on a failing write; [`CageError::Json`] if the
/// body cannot be serialized; [`CageError::Protocol`] if the body
/// exceeds [`MAX_FRAME_LEN`].
pub fn write_request<W: Write>(mut w: W, req: &Request) -> Result<(), CageError> {
    let body = serde_json::to_vec(req)?;
    let len = body_len_u32(body.len(), "request body")?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&body)?;
    Ok(())
}

/// Read a request: the inverse of [`write_request`].
///
/// # Errors
///
/// [`CageError::Protocol`] if the length exceeds [`MAX_FRAME_LEN`];
/// [`CageError::UnexpectedEof`] on truncation; [`CageError::Json`] on
/// a malformed body; [`CageError::Io`] on any other I/O failure.
pub fn read_request<R: Read>(mut r: R) -> Result<Request, CageError> {
    let len = read_len(&mut r, "request body")?;
    let mut body = vec![0_u8; len];
    read_exact(&mut r, &mut body)?;
    let req: Request = serde_json::from_slice(&body)?;
    Ok(req)
}

/// Write one `Output` frame containing `bytes`. An empty `bytes` is
/// permitted (and zero-cost).
///
/// # Errors
///
/// [`CageError::Io`] on a failing write; [`CageError::Protocol`] if
/// `bytes` exceeds [`MAX_FRAME_LEN`].
pub fn write_output<W: Write>(mut w: W, bytes: &[u8]) -> Result<(), CageError> {
    let len = body_len_u32(bytes.len(), "output chunk")?;
    w.write_all(&[TAG_OUTPUT])?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(bytes)?;
    Ok(())
}

/// Write the terminating `Exit` frame.
///
/// # Errors
///
/// [`CageError::Io`] on a failing write.
pub fn write_exit<W: Write>(mut w: W, code: u32) -> Result<(), CageError> {
    w.write_all(&[TAG_EXIT])?;
    w.write_all(&EXIT_PAYLOAD_LEN.to_be_bytes())?;
    w.write_all(&code.to_be_bytes())?;
    Ok(())
}

/// Read one response frame, classifying it.
///
/// # Errors
///
/// [`CageError::Protocol`] on an unknown tag, oversize length, or
/// mismatched `Exit` payload; [`CageError::UnexpectedEof`] on
/// truncation; [`CageError::Io`] on I/O.
pub fn read_response_frame<R: Read>(mut r: R) -> Result<ResponseFrame, CageError> {
    let mut tag_buf = [0_u8; 1];
    read_exact(&mut r, &mut tag_buf)?;
    let tag = tag_buf.first().copied().ok_or_else(unreachable_one_byte)?;
    let len = read_len(&mut r, "response frame")?;
    let mut payload = vec![0_u8; len];
    read_exact(&mut r, &mut payload)?;
    classify_frame(tag, &payload)
}

/// Dispatch a frame's payload by its tag. Kept separate so the read
/// path stays short.
fn classify_frame(tag: u8, payload: &[u8]) -> Result<ResponseFrame, CageError> {
    match tag {
        TAG_OUTPUT => Ok(ResponseFrame::Output(payload.to_vec())),
        TAG_EXIT => {
            let arr: [u8; 4] = payload.try_into().map_err(|_| {
                CageError::Protocol(format!(
                    "EXIT frame must carry 4 bytes, got {}",
                    payload.len()
                ))
            })?;
            Ok(ResponseFrame::Exit(u32::from_be_bytes(arr)))
        },
        unknown => Err(CageError::Protocol(format!(
            "unknown response frame tag: 0x{unknown:02x}"
        ))),
    }
}

/// Validate a payload length is `u32`-sized and under the cap, and
/// return it as a `u32` for the length prefix.
fn body_len_u32(len: usize, what: &str) -> Result<u32, CageError> {
    if len > MAX_FRAME_LEN {
        return Err(CageError::Protocol(format!(
            "{what} exceeds MAX_FRAME_LEN: {len}"
        )));
    }
    u32::try_from(len).map_err(|_| CageError::Protocol(format!("{what} length not u32: {len}")))
}

/// Read a `u32` length prefix and validate it against the cap, returning
/// the body length as `usize`.
fn read_len<R: Read>(r: &mut R, what: &str) -> Result<usize, CageError> {
    let raw = read_u32_be(r)?;
    let len = usize::try_from(raw)
        .map_err(|_| CageError::Protocol(format!("{what} length exceeds usize: {raw}")))?;
    if len > MAX_FRAME_LEN {
        return Err(CageError::Protocol(format!(
            "{what} exceeds MAX_FRAME_LEN: {len}"
        )));
    }
    Ok(len)
}

/// Read exactly `buf.len()` bytes; surface a mid-frame EOF distinctly
/// from a generic I/O error.
fn read_exact<R: Read>(mut r: R, buf: &mut [u8]) -> Result<(), CageError> {
    match r.read_exact(buf) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Err(CageError::UnexpectedEof),
        Err(e) => Err(CageError::Io(e)),
    }
}

/// Read a 4-byte big-endian `u32` length prefix.
fn read_u32_be<R: Read>(mut r: R) -> Result<u32, CageError> {
    let mut buf = [0_u8; 4];
    read_exact(&mut r, &mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

/// Constructor for an unreachable error: reading a `[u8; 1]` always
/// yields at least one byte if `read_exact` returned `Ok`. We avoid a
/// raw `unreachable!()` (denied) by returning a structured error if
/// the invariant is ever violated by a future refactor.
fn unreachable_one_byte() -> CageError {
    CageError::Protocol("internal: empty tag byte buffer after read_exact".to_owned())
}
