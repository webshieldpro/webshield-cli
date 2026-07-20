use crate::api::request_desc::RequestDesc;
use crate::api::table::DisplayTable;
use crate::i18n;
use crate::i18n::M;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::marker::PhantomData;

#[derive(Serialize, Deserialize)]
pub struct RecordItem<'a> {
    pub content: Cow<'a, str>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Serialize, Deserialize)]
pub(crate) enum ChangeType {
    DELETE,
}
/// A set of records sharing one name and type (rrset).
#[derive(Serialize, Deserialize)]
pub struct RRSet<'a> {
    pub name: Cow<'a, str>,
    #[serde(rename = "type")]
    pub rr_type: Cow<'a, str>,
    #[serde(default)]
    pub ttl: Option<i64>,
    #[serde(default)]
    pub records: Vec<RecordItem<'a>>,
    // /// Proxying flag (for A/AAAA/CNAME).
    #[serde(default)]
    pub proxied: bool,
    #[serde(rename = "changetype")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_type: Option<ChangeType>,
}
#[derive(Serialize, Deserialize)]
pub struct DnsRecords<'a> {
    #[serde(default)]
    pub rrsets: Vec<RRSet<'a>>,
    #[serde(default)]
    pub records_used: Option<i64>,
    #[serde(default)]
    pub records_limit: Option<i64>,
}

pub struct DNSDomainRecords;

impl RequestDesc for DNSDomainRecords {
    type Params = i64;
    type Request = ();
    type Response = DnsRecords<'static>;

    fn get_url(domain_id: Self::Params) -> impl AsRef<str> {
        format!("domains/{domain_id}/records")
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct DNSDomainRecordsPost<'s>(PhantomData<&'s ()>);

impl<'s> RequestDesc for DNSDomainRecordsPost<'s> {
    type Params = i64;
    type Request = DnsRecords<'s>;
    type Response = serde::de::IgnoredAny;

    fn get_url(domain_id: Self::Params) -> impl AsRef<str> {
        format!("domains/{domain_id}/records")
    }

    fn method() -> Method {
        Method::POST
    }
}
#[derive(Serialize)]
pub struct RRSetList<'a>(Vec<RRSet<'a>>);

impl<'a> From<Vec<RRSet<'a>>> for RRSetList<'a> {
    fn from(v: Vec<RRSet<'a>>) -> Self {
        RRSetList(v)
    }
}

impl<'a> DisplayTable for RRSetList<'a> {
    fn headers(&self) -> Vec<&'static str> {
        vec![
            i18n::tr(M::HName),
            i18n::tr(M::HType),
            i18n::tr(M::HTtl),
            i18n::tr(M::HProxy),
            i18n::tr(M::HValues),
        ]
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let yes = i18n::tr(M::Yes);
        self.0
            .iter()
            .map(|r| {
                let values = r
                    .records
                    .iter()
                    .map(|rec| rec.content.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![
                    r.name.to_string(),
                    r.rr_type.to_string(),
                    r.ttl.map(|t| t.to_string()).unwrap_or_default(),
                    if r.proxied { yes.into() } else { String::new() },
                    values,
                ]
            })
            .collect()
    }
}

#[derive(Deserialize, Serialize)]
pub struct DnssecResp {
    enabled: bool,
    algorithm: Option<String>,
    ds_records: Vec<String>,
    dnskey_records: Vec<String>,
    parent_ds_present: Option<bool>,
    parent_ds_error: Option<bool>,
}

impl DisplayTable for DnssecResp {
    fn headers(&self) -> Vec<&'static str> {
        todo!()
    }

    fn rows(&self) -> Vec<Vec<String>> {
        todo!()
    }
}

pub struct DnssecGet;

impl RequestDesc for DnssecGet {
    type Params = i64;
    type Request = ();
    type Response = DnssecResp;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/dnssec", id)
    }

    fn method() -> Method {
        Method::GET
    }
}

pub struct DnssecPost;

impl RequestDesc for DnssecPost {
    type Params = i64;
    type Request = ();
    type Response = DnssecResp;

    fn get_url(id: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/dnssec", id)
    }

    fn method() -> Method {
        Method::POST
    }
}

pub struct DnssecDelete;

impl RequestDesc for DnssecDelete {
    type Params = (i64, Option<String>);
    type Request = ();
    type Response = DnssecResp;

    fn get_url((id, query): Self::Params) -> impl AsRef<str> {
        let mut url = format!("domains/{}/dnssec", id);
        if let Some(ref q) = query {
            url.push_str(q);
        }
        url
    }

    fn method() -> Method {
        Method::DELETE
    }
}
