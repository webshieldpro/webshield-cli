use crate::api::run::Run;
use crate::api::table::ProgramRes;
use crate::i18n::LocaleCode;
use crate::util::context::Context;
use clap::Subcommand;

use crate::t;
use anyhow::Result;

#[derive(Subcommand)]
#[command(about = t!(cmd_lang))]
pub enum LanguageCommand {
    #[command(about = t!(cmd_lang_set))]
    Set {
        #[arg(value_enum)]
        locale: LocaleCode,
    },
    #[command(about = t!(cmd_lang_unset))]
    Unset,
}

impl Run for LanguageCommand {
    async fn run<'a>(self, ctx: &'a mut Context<'a>) -> Result<ProgramRes> {
        match self {
            Self::Set { locale } => change(ctx, Some(locale)).map(ProgramRes::from),
            Self::Unset => change(ctx, None).map(ProgramRes::from),
        }
    }
}

fn change(ctx: &mut Context<'_>, locale: Option<LocaleCode>) -> Result<()> {
    ctx.cfg.lang = locale;
    ctx.cfg.save()?;
    Ok(())
}
