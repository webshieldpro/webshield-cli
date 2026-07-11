use crate::api::get_url::MakeReq;
use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;
use std::marker::PhantomData;

pub struct StatBans;
#[derive(Deserialize)]
pub struct BanStats {

    pub bans: Vec<Value>, // TODO check ban signature
}

impl MakeReq for StatBans {
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

#[derive(Deserialize, Debug)]
pub struct Totals {
    requests: u64,
    status_2xx: u64,
    status_3xx: u64,
    status_4xx: u64,
    status_5xx: u64,
}


pub struct StatDomains;
#[derive(Deserialize, Debug)]
pub struct SummaryStats {
    domain: String,
    generated_at: String,
    range_seconds: u64,
    // series: Vec<_>,
    step_seconds: u32,
    totals: Totals,
}

impl MakeReq for StatDomains {
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
