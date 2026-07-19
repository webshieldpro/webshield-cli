/// Asks for confirmation (y/N) unless `--yes` was passed.
pub fn confirm(yes: bool, prompt: &str) -> anyhow::Result<()> {
    if yes {
        return Ok(());
    }
    use crate::i18n;
    use crate::i18n::M;
    use anyhow::bail;
    use std::io::Write;
    print!("{prompt} {}: ", i18n::tr(M::ConfirmSuffix));
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !matches!(&answer.trim().to_ascii_lowercase() as &str, "y" | "yes") {
        bail!(i18n::tr(M::ConfirmCancelled));
    }
    Ok(())
}
