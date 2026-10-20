//! Local-service authentication primitives (pure, no Tauri/DB dependency).
//!
//! If Intake Co-Pilot exposes a loopback listener, it must be token-gated, bound
//! to 127.0.0.1 on an ephemeral port, and must deny every browser origin.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

/// Constant-time byte comparison.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Generate a 32-byte session token from the OS CSPRNG, hex encoded.
///
/// Two v4 UUIDs provide 244 bits of `getrandom` entropy without an additional
/// randomness dependency.
pub fn generate_token() -> String {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Bind a loopback-only listener on an OS-assigned ephemeral port.
pub fn bind_ephemeral_loopback() -> std::io::Result<(TcpListener, u16)> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

#[derive(Debug, Clone)]
pub struct LocalServiceAuth {
    token: String,
}

impl LocalServiceAuth {
    pub fn new(token: String) -> Self {
        Self { token }
    }

    pub fn with_generated_token() -> Self {
        Self::new(generate_token())
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn is_authorized(&self, authorization_header: Option<&str>) -> bool {
        let Some(header) = authorization_header else {
            return false;
        };
        let Some(provided) = header.strip_prefix("Bearer ") else {
            return false;
        };
        ct_eq(provided.as_bytes(), self.token.as_bytes())
    }

    /// Deny all browser origins. The desktop frontend talks over Tauri IPC.
    pub fn is_allowed_origin(&self, origin: Option<&str>) -> bool {
        origin.is_none()
    }

    pub fn is_allowed_host(&self, host: &str) -> bool {
        let host = host.trim();
        host == "127.0.0.1" || host == "localhost" || host.starts_with("127.0.0.1:")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_is_32_bytes_hex() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn valid_bearer_is_authorized() {
        let auth = LocalServiceAuth::new("abc123".into());
        assert!(auth.is_authorized(Some("Bearer abc123")));
    }

    #[test]
    fn wrong_or_missing_bearer_is_rejected() {
        let auth = LocalServiceAuth::new("abc123".into());
        assert!(!auth.is_authorized(Some("Bearer wrong")));
        assert!(!auth.is_authorized(None));
        assert!(!auth.is_authorized(Some("abc123")));
    }

    #[test]
    fn all_browser_origins_are_denied() {
        let auth = LocalServiceAuth::new("abc123".into());
        assert!(!auth.is_allowed_origin(Some("http://evil.example")));
        assert!(!auth.is_allowed_origin(Some("http://localhost:3118")));
        assert!(auth.is_allowed_origin(None));
    }

    #[test]
    fn host_must_be_loopback() {
        let auth = LocalServiceAuth::new("abc123".into());
        assert!(auth.is_allowed_host("127.0.0.1"));
        assert!(auth.is_allowed_host("localhost"));
        assert!(auth.is_allowed_host("127.0.0.1:55001"));
        assert!(!auth.is_allowed_host("0.0.0.0"));
        assert!(!auth.is_allowed_host("example.com"));
    }

    #[test]
    fn ephemeral_bind_is_loopback_only() {
        let (listener, port) = bind_ephemeral_loopback().unwrap();
        assert!(port > 0);
        assert!(listener.local_addr().unwrap().ip().is_loopback());
    }

    #[test]
    fn ct_eq_matches_std_equality() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }
}
