use std::sync::Arc;

use crate::config::Project;
use crate::ext::anyhow::{Context, Result};
use crate::service::serve;
use crate::signal::{ProductChange, ProductSet};

pub async fn serve(proj: &Arc<Project>) -> Result<()> {
    super::build::build_proj(proj).await.dot()?;
    let server = serve::spawn(proj).await;
    // the server waits for the first product change before starting
    ProductChange::send(ProductSet::empty());

    server.await??;
    Ok(())
}
