use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::i18n;
use crate::i18n::M;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct StatBans;
#[derive(Deserialize, Serialize)]
pub struct BanStats {
    pub bans: Vec<Value>, // TODO check ban signature
}

impl RequestDesc for StatBans {
    type Params = (i64, String);
    type Request = ();
    type Response = BanStats;

    fn get_url(params: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/protection/bans?range={}", params.0, params.1)
    }

    fn method() -> Method {
        Method::GET
    }
}

impl DisplayTable for BanStats {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            i18n::tr(M::HIp),
            i18n::tr(M::HType),
            i18n::tr(M::HReason),
            i18n::tr(M::HLastSeen),
            i18n::tr(M::HRequests),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.bans
            .iter()
            .map(|b| {
                let s = |k: &str| b.get(k).map(|v| v.to_string()).unwrap_or_default(); // TODO conversion of Value to string
                vec![
                    s("ip"),
                    s("type"),
                    s("reason"),
                    s("last_seen"),
                    s("requests"),
                ]
            })
            .collect()
    }

    fn display_as_table(&self) {
        if self.bans.is_empty() {
            crate::output::info(i18n::tr(M::NoBans));
        } else {
            DisplayTable::display_as_table(self)
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Totals {
    requests: u64,
    status_2xx: u64,
    status_3xx: u64,
    status_4xx: u64,
    status_5xx: u64,
}

pub struct StatDomains;
#[derive(Deserialize, Serialize)]
pub struct SummaryStats {
    domain: String,
    generated_at: String,
    range_seconds: u64,
    // series: Vec<_>,
    step_seconds: u32,
    totals: Totals,
}

impl RequestDesc for StatDomains {
    type Params = (i64, String);
    type Request = ();
    type Response = SummaryStats;

    fn get_url(params: Self::Params) -> impl AsRef<str> {
        format!(
            "domains/{}/stats?range={}&step=2h", // TODO fix step param
            params.0, params.1
        )
    }

    fn method() -> Method {
        Method::GET
    }
}

impl DisplayTable for SummaryStats {
    fn headers(&self) -> Vec<&'static str> {
        todo!()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        todo!()
    }

    fn display_as_table(&self) {
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}
