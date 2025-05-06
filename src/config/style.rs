use super::{ProjectConfig, TailwindConfig};
use crate::service::site::{SiteFile, SourcedSiteFile};
use eyre::Result;

#[derive(Debug, Clone)]
pub struct StyleConfig {
    pub file: Option<SourcedSiteFile>,
    pub browserquery: String,
    pub tailwind: Option<TailwindConfig>,
    pub site_file: SiteFile,
}

impl StyleConfig {
    pub fn new(config: &ProjectConfig) -> Result<Self> {
        let site_rel = config
            .site_pkg_dir
            .join(&config.output_name)
            .with_extension("css");

        let site_file = SiteFile {
            dest: config.site_root.join(&site_rel),
            site: site_rel,
        };
        let style_file = config.style_file.as_ref().map(|file| {
            // relative to the configuration file
            let source = config.config_dir.join(file);
            let site = config
                .site_pkg_dir
                .join(&config.output_name)
                .with_extension("css");
            let dest = config.site_root.join(&site);
            SourcedSiteFile { source, dest, site }
        });
        Ok(Self {
            file: style_file,
            browserquery: config.browserquery.clone(),
            tailwind: TailwindConfig::new(config)?,
            site_file,
        })
    }
}
