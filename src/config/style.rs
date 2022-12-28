use crate::service::site::SourcedSiteFile;

use super::ProjectConfig;

#[derive(Debug)]
pub struct StyleConfig {
    pub file: SourcedSiteFile,
    pub browserquery: String,
}

impl StyleConfig {
    pub fn new(config: &ProjectConfig) -> Option<Self> {
        let Some(file) = &config.style_file else {
          return None;
        };
        let style_file = {
            // relative to the configuration file
            let source = config.config_dir.join(file);
            let site = config
                .site_pkg_dir
                .join(&config.output_name)
                .with_extension("css");
            let dest = config.site_root.join(&site);
            SourcedSiteFile { source, dest, site }
        };
        Some(Self {
            file: style_file,
            browserquery: config.browserquery.clone(),
        })
    }
}
