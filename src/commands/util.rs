use crate::t;
use clap::Args;
use std::num::NonZeroU32;

#[derive(Args)]
pub struct Page {
    #[arg(value_name = "PAGE(1..n)", help = t!(arg_page))]
    page: NonZeroU32,
}

impl From<Page> for u32 {
    fn from(v: Page) -> Self {
        v.page.get()
    }
}
