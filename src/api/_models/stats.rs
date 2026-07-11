use crate::api::get_url::MakeReq;
use reqwest::Method;

pub struct BanStats {
    bans: Vec<String>,
}

impl MakeReq for BanStats {
    type Params = (i64, &'static str);

    fn get_url(params: Self::Params) -> impl AsRef<str> {
        format!("domains/{}/protection/bans?range={}", params.0, params.1)
    }

    fn method() -> Method {
        Method::GET
    }
}
