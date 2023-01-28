use std::sync::Arc;

use crate::{
    compile,
    compile::ChangeSet,
    config::{Config, Project},
    ext::{
        anyhow::{Context, Result},
        fs,
    },
    signal::{Interrupt, Outcome},
};

pub async fn build_all(conf: &Config) -> Result<()> {
    for proj in &conf.projects {
        build_proj(proj).await?;
        if Interrupt::is_shutdown_requested().await {
            return Ok(());
        }
    }
    Ok(())
}

pub async fn build_proj(proj: &Arc<Project>) -> Result<()> {
    if proj.site.root_dir.exists() {
        fs::rm_dir_content(&proj.site.root_dir).await.dot()?;
    }
    let changes = ChangeSet::all_changes();

    if compile::front(proj, &changes).await.await?? == Outcome::Stopped {
        return Ok(());
    }

    if compile::assets(proj, &changes, true).await.await?? == Outcome::Stopped {
        return Ok(());
    }

    if compile::style(proj, &changes).await.await?? == Outcome::Stopped {
        return Ok(());
    }

    if compile::server(proj, &changes).await.await?? == Outcome::Stopped {
        return Ok(());
    }

    Ok(())
}
