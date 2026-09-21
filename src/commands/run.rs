use crate::commands::prog_res::ProgramRes;
use crate::util::context::Context;
use anyhow::Result;

pub trait Run {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes>;
}
