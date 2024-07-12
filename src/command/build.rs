use crate::Cli;
use color_eyre::Result;

pub async fn build_all(_cli: &Cli) -> Result<()> {
    println!("BUILDING!");
    Ok(())
}
