use reqwest::Method;

pub trait MakeReq {
    type Params;

    fn get_url(params: Self::Params) -> impl AsRef<str>;

    fn method() -> Method;
}
