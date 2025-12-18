use std::net::SocketAddr;

/// Parse bind address in host:port form.
pub fn parse_bind_addr(raw: &str) -> Result<SocketAddr, String> {
    raw.parse::<SocketAddr>()
        .map_err(|e| format!("invalid net.addr `{raw}`: {e}"))
}
