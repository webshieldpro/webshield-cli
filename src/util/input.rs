/// Asks for confirmation (y/N) unless `--yes` was passed.
pub fn confirm(yes: bool, prompt: &str) -> anyhow::Result<()> {
    if yes {
        return Ok(());
    }
    use crate::t;
    use anyhow::bail;
    use std::io::Write;
    print!("{prompt} {}: ", t!(confirm_suffix));
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !matches!(&answer.trim().to_ascii_lowercase() as &str, "y" | "yes") {
        bail!(t!(confirm_cancelled));
    }
    Ok(())
}
