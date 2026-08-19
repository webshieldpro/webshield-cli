//! Record types the CLI can write, plus normalization of their values to the
//! canonical form the API validates and stores (mirrors the dashboard).

use clap::ValueEnum;

#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
#[clap(rename_all = "uppercase")]
pub enum RrType {
    A,
    Aaaa,
    Cname,
    Txt,
    Mx,
    Ns,
    Ptr,
    Srv,
    Caa,
}

impl RrType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::Aaaa => "AAAA",
            Self::Cname => "CNAME",
            Self::Txt => "TXT",
            Self::Mx => "MX",
            Self::Ns => "NS",
            Self::Ptr => "PTR",
            Self::Srv => "SRV",
            Self::Caa => "CAA",
        }
    }

    /// CNAME/NS/PTR — an FQDN with a trailing dot, MX/SRV — a trailing dot on the
    /// target token, TXT — wrapped in quotes. Everything else goes as typed.
    pub fn normalize(&self, value: &str) -> String {
        let v = value.trim();
        if v.is_empty() {
            return v.to_string();
        }
        match self {
            Self::Cname | Self::Ns | Self::Ptr => ensure_trailing_dot(v),
            Self::Txt => {
                if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
                    v.to_string()
                } else {
                    format!("\"{v}\"")
                }
            }
            Self::Mx => dot_nth_token(v, 1),
            Self::Srv => dot_nth_token(v, 3),
            Self::A | Self::Aaaa | Self::Caa => v.to_string(),
        }
    }
}

pub fn ensure_trailing_dot(s: &str) -> String {
    if s.ends_with('.') {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

/// Appends a trailing dot to the n-th whitespace-separated token (the FQDN target
/// of MX/SRV). Values with fewer tokens are left as-is for the server to report
/// the format error.
fn dot_nth_token(value: &str, n: usize) -> String {
    let mut parts: Vec<String> = value.split_whitespace().map(str::to_string).collect();
    if let Some(target) = parts.get_mut(n) {
        *target = ensure_trailing_dot(target);
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_matches_backend_canonical_forms() {
        // CNAME/NS/PTR targets get a trailing dot (the backend rejects bare names).
        assert_eq!(
            RrType::Cname.normalize("target.example.com"),
            "target.example.com."
        );
        assert_eq!(
            RrType::Cname.normalize("target.example.com."),
            "target.example.com."
        );
        assert_eq!(RrType::Ns.normalize("ns1.example.com"), "ns1.example.com.");
        // MX/SRV: only the FQDN target token is dotted.
        assert_eq!(
            RrType::Mx.normalize("10 mail.example.com"),
            "10 mail.example.com."
        );
        assert_eq!(
            RrType::Mx.normalize("10 mail.example.com."),
            "10 mail.example.com."
        );
        assert_eq!(
            RrType::Srv.normalize("5 0 5060 sip.example.com"),
            "5 0 5060 sip.example.com."
        );
        // Malformed MX/SRV are passed through for the server to report the format error.
        assert_eq!(RrType::Mx.normalize("10"), "10");
        // TXT values are quoted, already-quoted values are untouched.
        assert_eq!(RrType::Txt.normalize("v=spf1 -all"), "\"v=spf1 -all\"");
        assert_eq!(RrType::Txt.normalize("\"v=spf1 -all\""), "\"v=spf1 -all\"");
        // Address records are untouched.
        assert_eq!(RrType::A.normalize("1.1.1.1"), "1.1.1.1");
        // Empty input stays empty instead of becoming a lone dot or `""`.
        assert_eq!(RrType::Cname.normalize("   "), "");
        assert_eq!(RrType::Txt.normalize(""), "");
    }
}
