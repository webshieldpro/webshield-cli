use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::i18n;
use crate::i18n::M;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
pub struct BillingBalance {
    pub default_currency: Option<String>,
    pub balances: HashMap<String, String>,
}

impl DisplayTable for BillingBalance {
    fn headers(&self) -> Vec<&'static str> {
        vec![i18n::tr(M::HCurrency), i18n::tr(M::HBalance)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        self.balances
            .iter()
            .map(|(cur, val)| vec![cur.clone(), val.clone()])
            .collect()
    }
}

pub struct Billing;

impl RequestDesc for Billing {
    type Params = ();

    type Request = ();

    type Response = BillingBalance;

    fn get_url(_: ()) -> impl AsRef<str> {
        "billing/balance"
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Deserialize, Serialize)]
pub struct BillingDomainUsage {
    pub domain_id: i64,
    pub period_start: String,
    pub period_end: String,
    pub bytes_used: i64,
    pub requests: i64,
    pub limit_gb: Option<i64>,
    pub limit_bytes: Option<i64>,
    pub used_ratio: Option<f64>,
    pub over_limit: bool,
    pub throttled: bool,
    pub overage_rate_kbps: Option<i64>,
    pub currency: Option<String>,
    pub tariff: Option<String>,
}

fn _option_to_string<S: ToString>(s: Option<S>) -> String {
    s.map(|s| s.to_string()).unwrap_or_else(|| "".to_string())
}

fn _bool_to_string(b: bool) -> String {
    if b { i18n::tr(M::Yes) } else { i18n::tr(M::No) }.to_string()
}

impl DisplayTable for BillingDomainUsage {
    fn headers(&self) -> Vec<&'static str> {
        vec![i18n::tr(M::HMetric), i18n::tr(M::HValue)]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        vec![
            vec!["bytes_used".into(), self.bytes_used.to_string()],
            vec!["limit_gb".into(), _option_to_string(self.limit_gb)],
            vec!["used_ratio".into(), _option_to_string(self.used_ratio)],
            vec!["over_limit".into(), _bool_to_string(self.over_limit)],
            vec!["throttled".into(), _bool_to_string(self.throttled)],
            vec!["tariff".into(), _option_to_string(self.tariff.clone())],
            vec!["currency".into(), _option_to_string(self.currency.clone())],
        ]
    }
}

pub struct BillingUsage;

impl RequestDesc for BillingUsage {
    type Params = i64;
    type Request = ();
    type Response = BillingDomainUsage;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/usage", id)
    }

    fn method() -> Method {
        Method::GET
    }
}

#[derive(Deserialize, Serialize)]
pub struct Tariff {
    name: String,
    currency: String,
    period: String,
    price: f64,
    traffic_limit_gb: Option<u32>,
    dns_records_limit: Option<u32>,
    traffic_overage_rate_kbps: Option<u32>,
    // features: Option<Value>,
    is_free: bool,
    is_active: bool,
    created_at: String,
}

// TODO
// let current = payload
// .get("current_tariff")
// .and_then(|t| t.get("name"))
// .and_then(Value::as_str)
// .unwrap_or("");
// let list = payload
// .get("tariffs")
// .and_then(Value::as_array)
// .cloned()
// .unwrap_or_default();
// let rows = list
// .iter()
// .map(|t| {
// let s = |k: &str| t.get(k).map(fmt_value).unwrap_or_default();
// let name = s("name");
// let marker = if name == current { "*" } else { "" };
// vec![marker.into(), name, s("price"), s("currency"), s("period")]
// })
// .collect();
// print_table(
// &[
// "",
// i18n::tr(M::HName),
// "price",
// i18n::tr(M::HCurrency),
// "period",
// ],
// rows,
// );

#[derive(Deserialize, Serialize)]
pub struct BillingTariffsGet {
    billing_enabled: bool,
    current_tariff: Option<Tariff>,
    tariffs: Vec<Tariff>,
}

impl DisplayTable for BillingTariffsGet {
    fn headers(&self) -> Vec<&'static str> {
        todo!()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        todo!()
    }

    fn display_as_table(&self) {
        // tmp
        println!("{}", serde_json::to_string_pretty(self).unwrap());
    }
}

pub struct BillingTariffs;

impl RequestDesc for BillingTariffs {
    type Params = i64;
    type Request = ();
    type Response = BillingTariffsGet;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/tariffs", id)
    }

    fn method() -> Method {
        Method::GET
    }
}
