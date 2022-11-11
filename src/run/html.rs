use crate::{util, Error, Reportable};
use simplelog as log;
use std::fs;

// markers used in the html template
const HEAD_MARKER: &str = "<!-- INJECT HEAD -->\n";
const BODY_MARKER: &str = "<!-- INJECT BODY -->";

// markers used in the rust template
const START_MARKER: &str = "--- START ---\n";
const MIDDLE_MARKER: &str = "--- MIDDLE ---\n";
const END_MARKER: &str = "--- END ---\n";

const HTML_HEAD_INSERT: &str = r##"<script type="module">import init from '/pkg/app.js';init('/pkg/app.wasm');</script>
    <link rel="preload" href="/pkg/app.wasm" as="fetch" type="application/wasm" crossorigin="">
    <link rel="stylesheet" href="/pkg/app.css">"
    <link rel="modulepreload" href="/pkg/app.js">"##;

pub struct Html {
    text: String,
}
impl Html {
    pub fn read(path: &str) -> Result<Self, Reportable> {
        Self::try_read(path).map_err(|e| e.file_context("read", path))
    }

    fn try_read(path: &str) -> Result<Self, Error> {
        let text = fs::read_to_string(path)?;
        if !text.find(HEAD_MARKER).is_some() {
            return Err(Error::MissingHtmlMarker(HEAD_MARKER));
        }
        if !text.find(BODY_MARKER).is_some() {
            return Err(Error::MissingHtmlMarker(BODY_MARKER));
        }
        log::trace!("Content of {path}:\n{text}");
        Ok(Self { text })
    }

    fn head(&self) -> &str {
        HTML_HEAD_INSERT
    }

    /// generate html for client side rendering
    pub fn generate_html(&self, file: &str) -> Result<(), Reportable> {
        let text = self
            .text
            .replace(HEAD_MARKER, &self.head())
            .replace(BODY_MARKER, "");

        log::debug!("Writing html to {file}");
        log::trace!("Html content\n{text}");

        util::write(file, &text)
    }

    /// generate rust for server side rendering
    pub fn generate_rust(&self, file: &str) -> Result<(), Reportable> {
        let rust = include_str!("generated.rs");

        let start_head = self.text.find(HEAD_MARKER).unwrap();
        let start = format!("{}", &self.text[0..start_head]);

        let end_head = start_head + HEAD_MARKER.len(); // it's ASCII so only 1 byte per char
        let start_body = self.text.find(BODY_MARKER).unwrap();
        let middle = &self.text[end_head..start_body];

        let end_body = start_body + BODY_MARKER.len();
        let end = &self.text[end_body..];

        let rust = rust
            .replace(START_MARKER, &start)
            .replace(MIDDLE_MARKER, middle)
            .replace(END_MARKER, end);

        log::debug!("Writing rust to {file}");
        log::trace!("Html content\n{rust}");

        util::write(file, &rust)
    }
}
