use reqwest::Method;
use crate::api::get_url::MakeReq;

pub struct DNSDomainRecords;

impl MakeReq for DNSDomainRecords {
    type Params = i64;
    type Request = ();
    type Response = ();

    fn get_url(domain_id: Self::Params) -> impl AsRef<str> {
            format!("domains/{domain_id}/records")
    }

    fn method() -> Method {
        todo!()
    }
}