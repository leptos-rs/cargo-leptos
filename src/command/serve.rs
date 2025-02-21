use std::sync::Arc;

use crate::config::Project;
use crate::internal_prelude::*;
use crate::service::serve;

pub async fn serve(proj: &Arc<Project>) -> Result<()> {
    if !super::build::build_proj(proj).await.dot()? {
        return Ok(());
    }
    let server = serve::spawn_oneshot(proj).await;
    server.await??;
    Ok(())
}
