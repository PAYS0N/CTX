//! Integration tests for the wire protocol. Hermetic: every transport
//! is `Vec<u8>` / `&[u8]`, no sockets, no processes.

use ctx_cage::error::CageError;
use ctx_cage::protocol::{
    read_request, read_response_frame, write_exit, write_output, write_request, Request,
    ResponseFrame, MAX_FRAME_LEN,
};

/// A handy sample request used across several tests.
fn sample_request() -> Request {
    Request {
        tool: "ctx-access".to_owned(),
        args: vec![
            "read".to_owned(),
            "crates/x/src/lib.rs".to_owned(),
            "--task-id".to_owned(),
            "demo".to_owned(),
        ],
    }
}

#[test]
fn request_round_trips_through_write_then_read() {
    let req = sample_request();
    let mut buf = Vec::new();
    write_request(&mut buf, &req).expect("write");
    let parsed = read_request(buf.as_slice()).expect("read");
    assert_eq!(parsed, req);
}

#[test]
fn response_stream_round_trips_output_chunks_then_exit() {
    let mut buf = Vec::new();
    write_output(&mut buf, b"hello, ").expect("o1");
    write_output(&mut buf, b"world").expect("o2");
    write_output(&mut buf, b"").expect("o3"); // empty chunk is legal
    write_exit(&mut buf, 0).expect("exit");

    let mut r = buf.as_slice();
    let f1 = read_response_frame(&mut r).expect("f1");
    let f2 = read_response_frame(&mut r).expect("f2");
    let f3 = read_response_frame(&mut r).expect("f3");
    let f4 = read_response_frame(&mut r).expect("f4");

    assert_eq!(f1, ResponseFrame::Output(b"hello, ".to_vec()));
    assert_eq!(f2, ResponseFrame::Output(b"world".to_vec()));
    assert_eq!(f3, ResponseFrame::Output(Vec::new()));
    assert_eq!(f4, ResponseFrame::Exit(0));
}

#[test]
fn exit_frame_carries_nonzero_code() {
    let mut buf = Vec::new();
    write_exit(&mut buf, 42).expect("exit");
    let frame = read_response_frame(buf.as_slice()).expect("read");
    assert_eq!(frame, ResponseFrame::Exit(42));
}

#[test]
fn unknown_tag_is_a_protocol_error() {
    // hand-craft a frame with an invalid tag (0xff) and a 0-length body.
    let bytes = [0xff_u8, 0, 0, 0, 0];
    let err = read_response_frame(bytes.as_slice()).expect_err("must reject unknown tag");
    let msg = err.to_string();
    assert!(msg.contains("unknown"), "got: {msg}");
}

#[test]
fn truncated_request_is_unexpected_eof() {
    // length prefix says 100 bytes but body is empty.
    let bytes = [0_u8, 0, 0, 100];
    let err = read_request(bytes.as_slice()).expect_err("must fail mid-body");
    assert!(matches!(err, CageError::UnexpectedEof), "got: {err}");
}

#[test]
fn oversize_length_is_rejected_without_allocating() {
    // length prefix = MAX_FRAME_LEN + 1; no body bytes follow but the
    // length check must reject before any read attempts the body.
    let oversize = u32::try_from(MAX_FRAME_LEN).expect("cap fits u32") + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&oversize.to_be_bytes());
    let err = read_request(bytes.as_slice()).expect_err("must reject by length");
    assert!(matches!(err, CageError::Protocol(_)), "got: {err}");
}

#[test]
fn exit_payload_must_be_four_bytes() {
    // Tag = EXIT (0x01), declared length = 3 (one short), three bytes
    // of payload — the read succeeds but classification must fail.
    let bytes = [1_u8, 0, 0, 0, 3, 0, 0, 0];
    let err = read_response_frame(bytes.as_slice()).expect_err("wrong exit payload");
    assert!(matches!(err, CageError::Protocol(_)), "got: {err}");
}

#[test]
fn malformed_request_json_surfaces_as_json_error() {
    // Length 2, body "{}" — valid JSON but missing required fields.
    let mut bytes = vec![0_u8, 0, 0, 2];
    bytes.extend_from_slice(b"{}");
    let err = read_request(bytes.as_slice()).expect_err("missing fields");
    assert!(matches!(err, CageError::Json(_)), "got: {err}");
}
