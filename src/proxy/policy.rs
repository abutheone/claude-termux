//! The proxy is a compatibility service, not an OS-enforced network sandbox.
use anyhow::{Result, ensure};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
};

const DEFAULT_HOSTS: &[&str] = &[
    "api.anthropic.com",
    "claude.ai",
    "claude.com",
    "platform.claude.com",
    "mcp-proxy.anthropic.com",
    "code.claude.com",
];

#[derive(Clone, Debug)]
pub struct Policy {
    hosts: HashSet<String>,
}
impl Policy {
    pub fn new(extra: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut hosts: HashSet<_> = DEFAULT_HOSTS.iter().map(|s| (*s).to_owned()).collect();
        for host in extra {
            validate_hostname(&host)?;
            hosts.insert(host);
        }
        Ok(Self { hosts })
    }
    pub fn permits(&self, host: &str) -> bool {
        self.hosts.contains(host)
    }
}

pub fn parse_authority(authority: &str) -> Result<&str> {
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("CONNECT requires host:443"))?;
    ensure!(port == "443", "only HTTPS port 443 is supported");
    validate_hostname(host)?;
    Ok(host)
}

fn validate_hostname(host: &str) -> Result<()> {
    ensure!(
        host.len() <= 253 && host.contains('.') && host.parse::<IpAddr>().is_err(),
        "an exact DNS hostname is required"
    );
    for label in host.split('.') {
        ensure!(
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "invalid hostname; use lowercase ASCII DNS labels"
        );
    }
    Ok(())
}

pub fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => public_ipv4(v4),
        IpAddr::V6(v6) => {
            // Admit current global-unicast space only. Exclude documentation,
            // transition and special-use ranges that can route to local services.
            let s = v6.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && s[1] < 0x0200)
                && !(s[0] == 0x2001 && s[1] == 0x0db8)
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && (s[1] & 0xf000) == 0)
        }
    }
}
fn public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 0 || b == 168 || (b == 88 && c == 99)))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_private_and_special_destinations() -> Result<()> {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "224.0.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "203.0.113.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "2001::1",
        ] {
            assert!(!public_address(ip.parse()?), "{ip}");
        }
        assert!(public_address("1.1.1.1".parse()?));
        assert!(public_address("2606:4700::1111".parse()?));
        Ok(())
    }
    #[test]
    fn rejects_ambiguous_authorities() {
        for authority in [
            "localhost:443",
            "127.0.0.1:443",
            "[::1]:443",
            "api.anthropic.com:80",
            "user@api.anthropic.com:443",
            "api.anthropic.com.:443",
            "API.ANTHROPIC.COM:443",
            "api.anthropic.com:0443",
        ] {
            assert!(parse_authority(authority).is_err(), "{authority}");
        }
    }
    #[test]
    fn uses_exact_hostnames() -> Result<()> {
        let policy = Policy::new([])?;
        assert!(policy.permits("api.anthropic.com"));
        assert!(!policy.permits("api.anthropic.com.evil.test"));
        Ok(())
    }
}
