use std::{borrow::Cow, env};

pub const ENV_VAR_LEPTOS_CARGO_GENERATE_VERSION: &str = "LEPTOS_CARGO_GENERATE_VERSION";
pub const ENV_VAR_LEPTOS_TAILWIND_VERSION: &str = "LEPTOS_TAILWIND_VERSION";
pub const ENV_VAR_LEPTOS_SASS_VERSION: &str = "LEPTOS_SASS_VERSION";
pub const ENV_VAR_LEPTOS_WASM_OPT_VERSION: &str = "LEPTOS_WASM_OPT_VERSION";

pub enum VersionConfig {
    Tailwind,
    WasmOpt,
    Sass,
    CargoGenerate,
}

impl VersionConfig {
    pub fn version<'a>(&self) -> Cow<'a, str> {
        env::var(self.env_var_version_name())
            .map(Cow::Owned)
            .unwrap_or_else(|_| self.default_version().into())
    }

    pub fn default_version(&self) -> &'static str {
        match self {
            Self::Tailwind => "v4.0.6",
            Self::WasmOpt => "version_117",
            Self::Sass => "1.83.4",
            Self::CargoGenerate => "v0.17.3",
        }
    }

    pub fn env_var_version_name(&self) -> &'static str {
        match self {
            Self::Tailwind => ENV_VAR_LEPTOS_TAILWIND_VERSION,
            Self::WasmOpt => ENV_VAR_LEPTOS_WASM_OPT_VERSION,
            Self::Sass => ENV_VAR_LEPTOS_SASS_VERSION,
            Self::CargoGenerate => ENV_VAR_LEPTOS_CARGO_GENERATE_VERSION,
        }
    }
}
