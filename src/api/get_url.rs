use reqwest::Method;
use serde::de::DeserializeOwned;

pub trait MakeReq {
    type Params;
    type Request;
    type Response: DeserializeOwned;

    fn get_url(params: Self::Params) -> impl AsRef<str>;

    fn method() -> Method;
}
