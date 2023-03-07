use std::sync::Arc;

use crate::{
    compile,
    compile::ChangeSet,
    config::{Config, Project},
    ext::{
        anyhow::{Context, Result},
        fs,
    },
};

pub async fn build_all(conf: &Config) -> Result<()> {
    for proj in &conf.projects {
        build_proj(proj).await?;
    }
    Ok(())
}

/// Build the project. Returns true if the build was successful
pub async fn build_proj(proj: &Arc<Project>) -> Result<bool> {
    if proj.site.root_dir.exists() {
        fs::rm_dir_content(&proj.site.root_dir).await.dot()?;
    }
    let changes = ChangeSet::all_changes();

    if !compile::front(proj, &changes).await.await??.is_success() {
        return Ok(false);
    }
    if !compile::assets(proj, &changes, true)
        .await
        .await??
        .is_success()
    {
        return Ok(false);
    }
    if !compile::style(proj, &changes).await.await??.is_success() {
        return Ok(false);
    }
    if !compile::server(proj, &changes).await.await??.is_success() {
        return Ok(false);
    }
    Ok(true)
}
