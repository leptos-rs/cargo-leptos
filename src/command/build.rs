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

pub async fn build_proj(proj: &Arc<Project>) -> Result<()> {
    if proj.site.root_dir.exists() {
        fs::rm_dir_content(&proj.site.root_dir).await.dot()?;
    }
    let changes = ChangeSet::all_changes();

    compile::server(proj, &changes).await.await??;
    compile::front(proj, &changes).await.await??;
    compile::assets(proj, &changes, true).await.await??;
    compile::style(proj, &changes).await.await??;
    Ok(())
}
