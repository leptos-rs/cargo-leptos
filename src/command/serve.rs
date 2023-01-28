use std::sync::Arc;

use crate::config::Project;
use crate::ext::anyhow::{Context, Result};
use crate::service::serve;
use crate::signal::Interrupt;

pub async fn serve(proj: &Arc<Project>) -> Result<()> {
    super::build::build_proj(proj).await.dot()?;
    if Interrupt::is_shutdown_requested().await {
        return Ok(());
    }

    let server = serve::spawn(proj).await;
    server.await??;
    Ok(())
}
