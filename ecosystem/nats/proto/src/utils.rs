use core::str::FromStr;
use nom::bytes::streaming::{tag, take, take_while1};
use nom::character::streaming::digit1;
use nom::character::{is_alphanumeric, is_space};
use nom::combinator::map_res;
use nom::sequence::terminated;
use simdutf8::basic::from_utf8;

/// ...
pub(crate) fn payload(buffer: &[u8]) -> nom::IResult<&[u8], &[u8]> {
    let (buffer, payload_size) = number(buffer)?;
    let (buffer, _) = newline(buffer)?;
    take(payload_size)(buffer)
}

/// ...
pub(crate) fn headers_and_payload(buffer: &[u8]) -> nom::IResult<&[u8], (&str, &[u8])> {
    let (buffer, header_size) = terminated(number, whitespace)(buffer)?;
    map_res(payload, move |headers_and_payload| {
        let (headers, payload) = headers_and_payload.split_at(header_size as usize);
        match from_utf8(headers) {
            Ok(headers) => Ok((headers, payload)),
            Err(err) => Err(err),
        }
    })(buffer)
}

/// Subject names are case-sensitive and must be non-empty alphanumeric strings with no embedded whitespace.
/// All ascii alphanumeric characters except spaces/tabs and separators which are . and > are allowed.
/// Subject names can be optionally token-delimited using the dot character (.).
/// A subject is comprised of 1 or more tokens.
/// Tokens are separated by . and can be any non space ascii alphanumeric character.
/// The full wildcard token > is only valid as the last token and matches all tokens past that point.
/// A token wildcard, * matches any token in the position it was listed.
/// Wildcard tokens should only be used in a wildcard capacity and not part of a literal token.
pub(crate) fn subject(buffer: &[u8]) -> nom::IResult<&[u8], &str> {
    // TODO: while(alphanumeric)|wildcard|fullwildcard delimited with .
    // TODO: use ranges where possible like is_alphanumeric
    let is_valid_subject =
        |c| is_alphanumeric(c) || c == b'_' || c == b'$' || c == b'.' || c == b'*' || c == b'>';
    map_res(take_while1(is_valid_subject), from_utf8)(buffer)
}

/// ...
pub(crate) fn number(buffer: &[u8]) -> nom::IResult<&[u8], u64> {
    map_res(map_res(digit1, from_utf8), u64::from_str)(buffer)
}

/// NATS uses CR followed by LF to terminate protocol messages.
pub(crate) fn newline(buffer: &[u8]) -> nom::IResult<&[u8], ()> {
    let (buffer, _) = tag("\r\n")(buffer)?;
    Ok((buffer, ()))
}

/// The fields of NATS protocol messages are delimited by whitespace characters (space) or (tab).
/// Multiple whitespace characters will be treated as a single field delimiter.
pub(crate) fn whitespace(buffer: &[u8]) -> nom::IResult<&[u8], ()> {
    let (buffer, _) = take_while1(is_space)(buffer)?;
    Ok((buffer, ()))
}
