use cargo_metadata::MetadataCommand;

use super::{BinConfig, Cli, LibConfig};

pub struct Config {
    is_workspace: bool,
    bin_config: Option<BinConfig>,
    lib_config: Option<LibConfig>,
}

impl From<Cli> for Config {
    fn from(cli: Cli) -> Self {
        if cli.opts.bin_only ^ cli.opts.lib_only {
            if cli.opts.bin_only {
                // Generate BinConfig from Cli and from BinOpts
                let bin_config = cli.generate_bin_config();

                Self {
                    lib_config: None,
                    bin_config: Some(bin_config),
                    is_workspace: cli.opts.is_workspace,
                }
            } else {
                // Generate LibConfig from Cli and from LibOpts
                let lib_config = cli.generate_lib_config();
                Self {
                    bin_config: None,
                    lib_config: Some(lib_config),
                    is_workspace: cli.opts.is_workspace,
                }
            }
        } else {
            let lib_config = cli.generate_lib_config();
            let bin_config = cli.generate_bin_config();

            Self {
                bin_config: Some(bin_config),
                lib_config: Some(lib_config),
                is_workspace: cli.opts.is_workspace,
            }
        }
    }
}

impl Cli {
    pub fn generate_bin_config(&self) -> BinConfig {
        // let metadata = MetadataCommand::new()
        //     .manifest_path(&self.manifest_path)
        //     .exec()
        //     .expect("Failed to read Cargo.toml at manifest-path. Are you sure it's valid?");
        BinConfig {}
    }
    pub fn generate_lib_config(&self) -> LibConfig {
        // let metadata = MetadataCommand::new()
        //     .manifest_path(&self.manifest_path)
        //     .exec()
        //     .expect("Failed to read Cargo.toml at manifest-path. Are you sure it's valid?");
        LibConfig {}
    }
}
