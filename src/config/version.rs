use std::{borrow::Cow, env};

pub const ENV_VAR_LEPTOS_TAILWIND_VERSION: &str = "LEPTOS_TAILWIND_VERSION";
pub const ENV_VAR_LEPTOS_SASS_VERSION: &str = "LEPTOS_SASS_VERSION";

pub enum VersionConfig {
    Tailwind,
    Sass,
}

impl VersionConfig {
    pub fn version<'a>(&self) -> Cow<'a, str> {
        env::var(self.env_var_version_name())
            .map(Cow::Owned)
            .unwrap_or_else(|_| self.default_version().into())
    }

    pub fn default_version(&self) -> &'static str {
        match self {
            Self::Tailwind => "v4.1.10",
            Self::Sass => "1.86.0",
        }
    }

    pub fn env_var_version_name(&self) -> &'static str {
        match self {
            Self::Tailwind => ENV_VAR_LEPTOS_TAILWIND_VERSION,
            Self::Sass => ENV_VAR_LEPTOS_SASS_VERSION,
        }
    }
}
