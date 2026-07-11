use crate::api::get_url::MakeReq;
use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;
use std::marker::PhantomData;

#[derive(Deserialize)]
pub struct BanStats<'s> {
    #[serde(skip)]
    _param_lifetime: PhantomData<&'s ()>,

    pub bans: Vec<Value>, // TODO check ban signature
}

impl<'s> MakeReq for BanStats<'s> {
    type Params = (i64, &'s str);

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

#[derive(Deserialize, Debug)]
pub struct SummaryStats<'s> {
    #[serde(skip)]
    _param_lifetime: PhantomData<&'s ()>,

    domain: String,
    generated_at: String,
    range_seconds: u64,
    // series: Vec<_>,
    step_seconds: u32,
    totals: Totals,
}

impl<'s> MakeReq for SummaryStats<'s> {
    type Params = (i64, &'s str);

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
