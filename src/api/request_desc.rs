use reqwest::Method;
use serde::de::DeserializeOwned;

pub trait RequestDesc {
    type Params;
    type Request;
    type Response: DeserializeOwned;

    fn get_url(params: Self::Params) -> impl AsRef<str>;

    fn method() -> Method;
}

/// A GET list endpoint paginated by DRF (`{count,next,previous,results}`).
/// The client follows `next` and concatenates `results` into a `Vec<Item>`;
/// a bare JSON array is accepted too.
pub trait ListRequestDesc {
    type Params;
    type Item: DeserializeOwned;

    fn get_url(params: Self::Params) -> impl AsRef<str>;
}
