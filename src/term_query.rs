//! Terminal background-color query helpers (OSC 11).
//!
//! At startup `markless` sends an OSC 11 query asking the terminal for
//! its background color, then picks a syntax-highlight theme readable
//! against it. The terminal replies with `ESC ] 11 ; rgb:RRRR/GGGG/BBBB`
//! terminated by `BEL` or `ESC \`.
//!
//! [`read_osc_response`] runs the query and the bounded read on the
//! calling thread against a non-blocking fd, which is what makes it
//! safe to call from `main`: no reader can outlive the function and
//! steal subsequent keystrokes from crossterm (issue #53).

use std::io::{ErrorKind, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// OSC 11 query payload: `ESC ] 11 ; ? BEL`.
const OSC11_QUERY: &[u8] = b"\x1b]11;?\x07";

/// Send an OSC 11 query on `io` and collect the reply until a terminator
/// (`BEL` or `ESC \`) arrives or `timeout` elapses.
///
/// `io` must already be in non-blocking mode. Returns the raw bytes
/// received, empty on timeout or error.
pub fn read_osc_response<S: Read + Write>(io: &mut S, timeout: Duration) -> Vec<u8> {
    if io.write_all(OSC11_QUERY).is_err() {
        return Vec::new();
    }
    let _ = io.flush();

    let deadline = Instant::now() + timeout;
    let mut collected: Vec<u8> = Vec::with_capacity(64);
    let mut buf = [0u8; 256];

    while Instant::now() < deadline {
        match io.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                collected.extend_from_slice(&buf[..n]);
                if has_osc_terminator(&collected) {
                    break;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }

    collected
}

fn has_osc_terminator(bytes: &[u8]) -> bool {
    bytes.contains(&b'\x07') || bytes.windows(2).any(|w| w == b"\x1b\\")
}

/// Parse an OSC 11 reply payload into an 8-bit-per-channel RGB triple.
/// Returns `None` if the reply doesn't contain an `rgb:` section or is
/// otherwise malformed.
pub fn parse_osc11_reply(reply: &str) -> Option<(u8, u8, u8)> {
    let start = reply.find("rgb:")?;
    let data = reply.get(start + 4..)?;
    let mut parts = data.split(['/', '\x07', '\x1b']);
    let r = parts.next()?;
    let g = parts.next()?;
    let b = parts.next()?;
    Some((
        parse_osc_component(r)?,
        parse_osc_component(g)?,
        parse_osc_component(b)?,
    ))
}

/// Parse a single hex color component. Terminals usually emit four hex
/// digits per channel; some emit two.
fn parse_osc_component(s: &str) -> Option<u8> {
    let hex = s.trim();
    if hex.len() >= 4 {
        let head = hex.get(..4)?;
        let v = u16::from_str_radix(head, 16).ok()?;
        u8::try_from(v >> 8).ok()
    } else if hex.len() == 2 {
        u8::from_str_radix(hex, 16).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_osc11_reply_handles_four_digit_components() {
        let reply = "\x1b]11;rgb:1818/2828/3838\x07";
        assert_eq!(parse_osc11_reply(reply), Some((0x18, 0x28, 0x38)));
    }

    #[test]
    fn parse_osc11_reply_handles_two_digit_components() {
        let reply = "\x1b]11;rgb:18/28/38\x07";
        assert_eq!(parse_osc11_reply(reply), Some((0x18, 0x28, 0x38)));
    }

    #[test]
    fn parse_osc11_reply_handles_st_terminator() {
        let reply = "\x1b]11;rgb:0000/0000/0000\x1b\\";
        assert_eq!(parse_osc11_reply(reply), Some((0x00, 0x00, 0x00)));
    }

    #[test]
    fn parse_osc11_reply_rejects_missing_rgb_prefix() {
        assert_eq!(parse_osc11_reply("\x1b]11;1234/5678/9abc\x07"), None);
    }

    #[cfg(unix)]
    fn pair() -> (
        std::os::unix::net::UnixStream,
        std::os::unix::net::UnixStream,
    ) {
        let (a, b) = std::os::unix::net::UnixStream::pair().expect("socket pair");
        a.set_nonblocking(true).expect("set_nonblocking");
        (a, b)
    }

    #[cfg(unix)]
    #[test]
    fn read_osc_response_returns_promptly_on_silent_peer() {
        let (mut client, _server) = pair();
        let start = Instant::now();
        let collected = read_osc_response(&mut client, Duration::from_millis(50));
        let elapsed = start.elapsed();
        assert!(collected.is_empty(), "got {collected:?}");
        assert!(
            elapsed < Duration::from_millis(250),
            "took {elapsed:?}, expected to bail out near the 50 ms deadline",
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_osc_response_collects_complete_reply() {
        let (mut client, mut server) = pair();
        std::thread::spawn(move || {
            let mut buf = [0u8; 32];
            let _ = server.read(&mut buf);
            let _ = server.write_all(b"\x1b]11;rgb:1818/2828/3838\x07");
        });
        let collected = read_osc_response(&mut client, Duration::from_millis(500));
        assert!(collected.windows(4).any(|w| w == b"rgb:"));
        assert!(collected.contains(&b'\x07'));
        let text = String::from_utf8_lossy(&collected);
        assert_eq!(parse_osc11_reply(&text), Some((0x18, 0x28, 0x38)));
    }

    #[cfg(unix)]
    #[test]
    fn read_osc_response_reassembles_split_reads() {
        let (mut client, mut server) = pair();
        std::thread::spawn(move || {
            let mut buf = [0u8; 32];
            let _ = server.read(&mut buf);
            let _ = server.write_all(b"\x1b]11;rgb:");
            std::thread::sleep(Duration::from_millis(15));
            let _ = server.write_all(b"1818/2828/3838\x07");
        });
        let collected = read_osc_response(&mut client, Duration::from_millis(500));
        let text = String::from_utf8_lossy(&collected);
        assert_eq!(parse_osc11_reply(&text), Some((0x18, 0x28, 0x38)));
    }

    /// Regression test for issue #53: bytes written to the peer after
    /// `read_osc_response` returns must still be readable by the caller.
    /// A stray reader holding the fd would consume them instead.
    #[cfg(unix)]
    #[test]
    fn read_osc_response_does_not_leave_orphan_consumer() {
        let (mut client, mut server) = pair();
        let _ = read_osc_response(&mut client, Duration::from_millis(20));

        server
            .write_all(b"post-timeout bytes")
            .expect("server write");
        client.set_nonblocking(false).expect("set blocking");
        let mut buf = [0u8; 64];
        let n = client.read(&mut buf).expect("read after timeout");
        assert_eq!(&buf[..n], b"post-timeout bytes");
    }
}
