//! Minimal hand-rolled protobuf wire-format codec for the two messages needed by
//! the token-refresh flow on `POST /v3/auth/login`:
//!
//! ```text
//! AuthGatewayRequest { oneof factor { RefreshAuth refresh_auth = 10; } }
//! RefreshAuth        { string refresh_token = 1; }
//!
//! AuthGatewayResponse {
//!     MetaProto meta = 1;
//!     ErrorProto error = 2;          // { int32 code=1; string message=2; }
//!     oneof data { LoginResult login_result = 8; ... }
//! }
//! LoginResult {
//!     string refresh_token = 1;
//!     string auth_token   = 2;
//!     enum   captcha      = 3;
//!     string user_id      = 4;
//!     Int64Value auth_token_ttl = 5; // { int64 value = 1; }
//! }
//! ```
//!
//! Only the fields we need are read; unknown fields are skipped.

use crate::api::types::RefreshResult;

const WIRE_VARINT: u32 = 0;
const WIRE_LEN: u32 = 2;

pub fn encode_refresh_request(refresh_token: &str) -> Vec<u8> {
    // RefreshAuth { refresh_token = 1 }
    let mut inner = Vec::with_capacity(refresh_token.len() + 2);
    inner.push(0x0A); // field 1, wire type LEN
    write_varint(&mut inner, refresh_token.len() as u64);
    inner.extend_from_slice(refresh_token.as_bytes());

    // AuthGatewayRequest { refresh_auth = 10 }
    let mut out = Vec::with_capacity(inner.len() + 2);
    out.push((10u32 << 3 | WIRE_LEN) as u8); // field 10, wire type LEN
    write_varint(&mut out, inner.len() as u64);
    out.extend_from_slice(&inner);
    out
}

pub fn decode_refresh_response(bytes: &[u8]) -> Result<RefreshResult, String> {
    let mut result = RefreshResult::default();
    let mut error_message: Option<String> = None;

    let mut pos = 0usize;
    while pos < bytes.len() {
        let (tag, next) = read_varint(bytes, pos).ok_or("truncated varint")?;
        pos = next;
        let field = (tag >> 3) as u32;
        let wire = (tag & 0x7) as u32;

        match wire {
            WIRE_VARINT => {
                let (_, next) = read_varint(bytes, pos).ok_or("truncated varint")?;
                pos = next;
            }
            WIRE_LEN => {
                let (len, next) = read_varint(bytes, pos).ok_or("truncated len")?;
                pos = next;
                let start = pos;
                let end = start
                    .checked_add(len as usize)
                    .filter(|e| *e <= bytes.len())
                    .ok_or("truncated length-delimited field")?;
                let value = &bytes[start..end];
                pos = end;

                match field {
                    2 => {
                        // ErrorProto
                        if let Some(msg) = extract_error_message(value) {
                            error_message = Some(msg);
                        }
                    }
                    8 => {
                        // LoginResult
                        decode_login_result(value, &mut result)?;
                    }
                    _ => {}
                }
            }
            _ => {
                // Groups and fixed-width are not used by these messages.
                return Err(format!("unsupported wire type {wire} for field {field}"));
            }
        }
    }

    if let Some(msg) = error_message {
        // Only fail if we didn't also get a login result.
        if result.auth_token.is_empty() {
            return Err(format!("auth gateway error: {msg}"));
        }
    }

    if result.auth_token.is_empty() {
        return Err("auth gateway response contained no login_result.auth_token".into());
    }
    Ok(result)
}

fn decode_login_result(bytes: &[u8], out: &mut RefreshResult) -> Result<(), String> {
    let mut pos = 0usize;
    while pos < bytes.len() {
        let (tag, next) = match read_varint(bytes, pos) {
            Some(t) => t,
            None => return Err("truncated login varint".into()),
        };
        pos = next;
        let field = (tag >> 3) as u32;
        let wire = (tag & 0x7) as u32;

        match wire {
            WIRE_VARINT => {
                let (_, next) = match read_varint(bytes, pos) {
                    Some(t) => t,
                    None => return Err("truncated login varint".into()),
                };
                pos = next;
            }
            WIRE_LEN => {
                let (len, next) = match read_varint(bytes, pos) {
                    Some(t) => t,
                    None => return Err("truncated login varint".into()),
                };
                pos = next;
                let start = pos;
                let end = start
                    .checked_add(len as usize)
                    .filter(|e| *e <= bytes.len())
                    .ok_or("truncated login field")?;
                let value = &bytes[start..end];
                pos = end;

                match field {
                    1 => out.refresh_token = Some(String::from_utf8_lossy(value).into_owned()),
                    2 => out.auth_token = String::from_utf8_lossy(value).into_owned(),
                    4 => out.user_id = Some(String::from_utf8_lossy(value).into_owned()),
                    5 => {
                        // Int64Value { int64 value = 1 }
                        if let Some(v) = read_first_varint(value) {
                            out.ttl_ms = Some(v as i64);
                        }
                    }
                    _ => {}
                }
            }
            _ => return Err("unsupported login field".into()),
        }
    }
    Ok(())
}

fn extract_error_message(bytes: &[u8]) -> Option<String> {
    let mut pos = 0usize;
    while pos < bytes.len() {
        let (tag, next) = read_varint(bytes, pos)?;
        pos = next;
        let field = (tag >> 3) as u32;
        let wire = (tag & 0x7) as u32;
        match wire {
            WIRE_VARINT => {
                let (_, next) = read_varint(bytes, pos)?;
                pos = next;
            }
            WIRE_LEN => {
                let (len, next) = read_varint(bytes, pos)?;
                pos = next;
                let start = pos;
                let end = start
                    .checked_add(len as usize)
                    .filter(|e| *e <= bytes.len())?;
                let value = &bytes[start..end];
                pos = end;
                if field == 2 {
                    return Some(String::from_utf8_lossy(value).into_owned());
                }
            }
            _ => break,
        }
    }
    None
}

fn read_first_varint(bytes: &[u8]) -> Option<u64> {
    let (tag, pos) = read_varint(bytes, 0)?;
    if tag != 8 {
        return None;
    }
    read_varint(bytes, pos).map(|(value, _)| value)
}

fn read_varint(bytes: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(pos)?;
        pos += 1;
        if shift == 63 && byte > 1 {
            return None;
        }
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, pos));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn write_varint(buf: &mut Vec<u8>, mut mut_val: u64) {
    loop {
        let mut byte = (mut_val & 0x7f) as u8;
        mut_val >>= 7;
        if mut_val != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if mut_val == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_refresh_shape() {
        let bytes = encode_refresh_request("tok");
        // field 10 LEN, length 3 (0x0A, len 3, 't','o','k'), wrapped: 0x52, 0x05, 0x0A, 0x03, t,o,k
        assert_eq!(bytes, vec![0x52, 0x05, 0x0A, 0x03, b't', b'o', b'k']);
    }

    #[test]
    fn decode_login_result_minimal() {
        // LoginResult with auth_token="abc" (field 2), refresh_token="r" (field 1)
        let login = {
            let mut v = vec![0x0A, 1, b'r', 0x12, 3];
            v.extend_from_slice(b"abc"); // field2 "abc"
            v
        };
        // AuthGatewayResponse { login_result = 8 }
        let mut resp = Vec::new();
        resp.push(0x42); // field 8 LEN
        write_varint(&mut resp, login.len() as u64);
        resp.extend_from_slice(&login);

        let out = decode_refresh_response(&resp).unwrap();
        assert_eq!(out.auth_token, "abc");
        assert_eq!(out.refresh_token.as_deref(), Some("r"));
    }

    #[test]
    fn decode_error() {
        // ErrorProto { code=1 (varint), message="denied" }
        let mut err = vec![0x08, 1, 0x12, 6];
        err.extend_from_slice(b"denied");
        let mut resp = Vec::new();
        resp.push(0x12); // field2 LEN
        write_varint(&mut resp, err.len() as u64);
        resp.extend_from_slice(&err);
        let res = decode_refresh_response(&resp);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("denied"));
    }
}

#[cfg(test)]
mod robustness_tests {
    use super::*;
    #[test]
    fn nested_integer_reads_value_not_tag() {
        assert_eq!(read_first_varint(&[8, 150, 1]), Some(150));
    }
    #[test]
    fn rejects_overflow_and_truncated_login_field() {
        assert!(read_varint(&[255; 11], 0).is_none());
        assert!(decode_refresh_response(&[0x42, 4, 0x12, 10, b'a', b'b']).is_err());
    }
}
