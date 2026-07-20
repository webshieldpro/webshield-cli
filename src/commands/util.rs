use clap::Args;

#[derive(Args)]
pub struct Page {
    #[arg(value_name = "PAGE(1..n)")]
    page: u32,
}

impl From<Page> for u32 {
    fn from(v: Page) -> Self {
        v.page
    }
}
