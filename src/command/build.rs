use std::sync::Arc;

use crate::ext::compress;
use crate::{
    compile,
    compile::ChangeSet,
    config::{Config, Project},
    ext::{
        anyhow::{anyhow, Context, Result},
        fs,
    },
};

pub async fn build_all(conf: &Config) -> Result<()> {
    let mut first_failed_project = None;

    for proj in &conf.projects {
        log::debug!("Building project: {}, {}", proj.name, proj.working_dir);
        if !build_proj(proj).await? && first_failed_project.is_none() {
            first_failed_project = Some(proj);
        }
    }

    if let Some(proj) = first_failed_project {
        Err(anyhow!("Failed to build {}", proj.name))
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

    if !proj.lib.skipped {
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

        if proj.hash_files {
            compile::add_hashes_to_site(proj)?;
        }

        // it is important to do the precompression of the static files before building the
        // server to make it possible to include them as assets into the binary itself
        if proj.release
            && proj.precompress
            && compress::compress_static_files(proj.site.root_dir.clone().into())
                .await
                .is_err()
        {
            return Ok(false);
        }
    }

    if !proj.bin.skipped && !compile::server(proj, &changes).await.await??.is_success() {
        return Ok(false);
    }

    Ok(true)
}
