use super::policy;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use subtle::ConstantTimeEq;

pub const MAX_HEADERS: usize = 8192;

pub enum Request<'a> {
    Incomplete,
    Unauthorized,
    Tunnel { hostname: &'a str, consumed: usize },
}

pub fn parse<'a>(bytes: &'a [u8], expected_auth: &[u8]) -> Result<Request<'a>> {
    ensure!(bytes.len() <= MAX_HEADERS, "request headers too large");
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut request = httparse::Request::new(&mut headers);
    let httparse::Status::Complete(consumed) = request.parse(bytes)? else {
        return Ok(Request::Incomplete);
    };
    ensure!(
        request.method == Some("CONNECT") && request.version == Some(1),
        "only HTTP/1.1 CONNECT is supported"
    );
    let mut credentials = request
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("proxy-authorization"));
    let supplied = credentials.next();
    ensure!(credentials.next().is_none(), "duplicate proxy credentials");
    // Authenticate before validating, resolving, or connecting to any destination.
    if !supplied.is_some_and(|h| bool::from(h.value.ct_eq(expected_auth))) {
        return Ok(Request::Unauthorized);
    }
    ensure!(
        !request
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("content-length")
                || h.name.eq_ignore_ascii_case("transfer-encoding")),
        "CONNECT must not have an HTTP body"
    );
    let authority = request.path.context("missing CONNECT authority")?;
    let hostname = policy::parse_authority(authority)?;
    let mut host_headers = request
        .headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("host"));
    if let Some(host) = host_headers.next() {
        ensure!(
            host.value == authority.as_bytes(),
            "Host and CONNECT authority disagree"
        );
    }
    ensure!(host_headers.next().is_none(), "duplicate Host header");
    Ok(Request::Tunnel { hostname, consumed })
}

pub fn authorization(token: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("claude:{token}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_authentication_before_destination_processing() -> Result<()> {
        let request =
            b"CONNECT forbidden.invalid:443 HTTP/1.1\r\nHost: forbidden.invalid:443\r\n\r\n";
        assert!(matches!(
            parse(request, authorization("secret").as_bytes())?,
            Request::Unauthorized
        ));
        Ok(())
    }
    #[test]
    fn rejects_header_smuggling() {
        let auth = authorization("secret");
        for extra in [
            "Content-Length: 0\r\n",
            "Transfer-Encoding: chunked\r\n",
            "Host: evil.test:443\r\n",
            &format!("Proxy-Authorization: {auth}\r\n"),
        ] {
            let request = format!(
                "CONNECT api.anthropic.com:443 HTTP/1.1\r\nHost: api.anthropic.com:443\r\nProxy-Authorization: {auth}\r\n{extra}\r\n"
            );
            assert!(parse(request.as_bytes(), auth.as_bytes()).is_err());
        }
    }
    #[test]
    fn preserves_early_tunnel_bytes() -> Result<()> {
        let auth = authorization("secret");
        let request = format!(
            "CONNECT api.anthropic.com:443 HTTP/1.1\r\nProxy-Authorization: {auth}\r\n\r\nTLS"
        );
        let Request::Tunnel { hostname, consumed } = parse(request.as_bytes(), auth.as_bytes())?
        else {
            anyhow::bail!("expected tunnel")
        };
        assert_eq!(hostname, "api.anthropic.com");
        assert_eq!(&request.as_bytes()[consumed..], b"TLS");
        Ok(())
    }
}
