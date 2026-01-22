//! Check command handler.

use anyhow::Result;

use crate::cli::CheckArgs;

pub fn handle_check(_args: &CheckArgs) -> Result<()> {
    println!("check: not yet implemented");
    Ok(())
}
