use crate::{config::Config, util};
use anyhow::{ensure, Context, Result};
use simplelog as log;
use std::fs;
use util::StrAdditions;

// markers used in the html template
const HEAD_MARKER: &str = "<!-- INJECT HEAD -->\n";
const BODY_MARKER: &str = "<!-- INJECT BODY -->";

// markers used in the rust template
const START_MARKER: &str = "--- START ---\n";
const MIDDLE_MARKER: &str = "--- MIDDLE ---\n";
const END_MARKER: &str = "--- END ---\n";

const HTML_HEAD_INSERT: &str = r##"
    <script type="module">import init from '/pkg/app.js';init('/pkg/app_bg.wasm');</script>
    <link rel="preload" href="/pkg/app_bg.wasm" as="fetch" type="application/wasm" crossorigin="">
    <link rel="stylesheet" href="/pkg/app.css">
    <link rel="modulepreload" href="/pkg/app.js">"##;

pub struct Html {
    text: String,
}
impl Html {
    pub fn read(path: &str) -> Result<Self> {
        Self::try_read(path).context(format!("read {path}"))
    }

    fn try_read(path: &str) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        ensure!(
            text.find(HEAD_MARKER).is_some(),
            format!("Missing Html marker {HEAD_MARKER}")
        );
        ensure!(
            text.find(BODY_MARKER).is_some(),
            format!("Missing Html marker {BODY_MARKER}")
        );
        log::trace!("Content of {path}:\n{text}");
        Ok(Self { text })
    }

    fn head(&self) -> &str {
        HTML_HEAD_INSERT
    }

    /// generate html for client side rendering
    pub fn generate_html(&self) -> Result<()> {
        let file = util::mkdirs("target/site/")?.with("index.html");

        let text = self
            .text
            .replace(HEAD_MARKER, &self.head())
            .replace(BODY_MARKER, "");

        log::debug!("Writing html to {file}");
        log::trace!("Html content\n{text}");

        util::write(&file, &text)
    }

    /// generate rust for server side rendering
    pub fn generate_rust(&self, config: &Config) -> Result<()> {
        let file = &config.gen_path;

        let rust = include_str!("generated.rs");

        let start_head = self.text.find(HEAD_MARKER).unwrap();
        let start = format!("{}{}", &self.text[0..start_head].trim(), self.head());

        let end_head = start_head + HEAD_MARKER.len(); // it's ASCII so only 1 byte per char
        let start_body = self.text.find(BODY_MARKER).unwrap();
        let middle = format!("  {}", &self.text[end_head..start_body].trim());

        let end_body = start_body + BODY_MARKER.len();
        let end = format!("  {}", &self.text[end_body..].trim());

        let rust = rust
            .replace(START_MARKER, &start)
            .replace(MIDDLE_MARKER, &middle)
            .replace(END_MARKER, &end);

        log::debug!("Writing rust to {file}");
        log::trace!("Html content\n{rust}");

        util::write(&file, &rust)
    }
}
