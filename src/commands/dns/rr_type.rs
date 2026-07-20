use clap::ValueEnum;

#[derive(ValueEnum, Clone)]
#[clap(rename_all = "uppercase")]
pub enum RrType {
    CNAME,
    A,
    AAAA,
}

impl RrType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RrType::CNAME => "CNAME",
            RrType::A => "A",
            RrType::AAAA => "AAAA",
        }
    }

    pub fn normalize(&self, value: &str) -> String {
        let v = value.trim();
        if v.is_empty() {
            return v.to_string();
        }
        match self {
            Self::CNAME => ensure_trailing_dot(v),
            Self::A | Self::AAAA => v.to_string(),
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
