use std::sync::Arc;

use crate::ext::compress;
use crate::internal_prelude::*;
use crate::{
    compile,
    compile::ChangeSet,
    config::{Config, Project},
    ext::fs,
};

pub async fn build_all(conf: &Config) -> Result<()> {
    let mut first_failed_project = None;

    for proj in &conf.projects {
        debug!("Building project: {}, {}", proj.name, proj.working_dir);
        if !build_proj(proj).await? && first_failed_project.is_none() {
            first_failed_project = Some(proj);
        }
    }

    if let Some(proj) = first_failed_project {
        Err(eyre!("Failed to build {}", proj.name))
    } else {
        Ok(())
    }
}

/// Build the project. Returns true if the build was successful
pub async fn build_proj(proj: &Arc<Project>) -> Result<bool> {
    if proj.site.root_dir.exists() {
        fs::rm_dir_content(&proj.site.root_dir).await.dot()?;
    }
    let changes = ChangeSet::all_changes();

    let mut success = true;

    if !compile::front(proj, &changes).await.await??.is_success() {
        success = false;
    }
    if !compile::assets(proj, &changes).await.await??.is_success() {
        success = false;
    }
    if !compile::style(proj, &changes).await.await??.is_success() {
        success = false;
    }

    if !success {
        return Ok(false);
    }

    if proj.hash_files {
        compile::add_hashes_to_site(proj)?;
    }

    // it is important to do the precompression of the static files before building the
    // server to make it possible to include them as assets into the binary itself
    if proj.release && proj.precompress {
        compress::compress_static_files(proj.site.root_dir.clone().into()).await?;
    }

    if !compile::server(proj, &changes).await.await??.is_success() {
        return Ok(false);
    }

    Ok(true)
}
