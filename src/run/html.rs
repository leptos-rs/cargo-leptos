use crate::{fs, fs::PathBufAdditions, Config};
use anyhow_ext::{ensure, Context, Result};
use regex::Regex;
use std::path::PathBuf;

lazy_static::lazy_static! {
    static ref HEAD_RE: Regex = Regex::new(&format!(r"\s*{HEAD_MARKER}\s*?")).unwrap();
    static ref BODY_RE: Regex = Regex::new(&format!(r"\s*{BODY_MARKER}\s*")).unwrap();
    static ref START_RE: Regex = Regex::new(&format!(r"\s*{START_MARKER}\s*\n?")).unwrap();
    static ref RELOAD_RE: Regex = Regex::new(&format!(r"\s*{RELOAD_MARKER}\s*\n?")).unwrap();
    static ref MIDDLE_RE: Regex = Regex::new(&format!(r"\s*{MIDDLE_MARKER}\s*\n?")).unwrap();
    static ref END_RE: Regex = Regex::new(&format!(r"\s*{END_MARKER}\s*\n?")).unwrap();
}

// markers used in the html template
const HEAD_MARKER: &str = "<!-- INJECT HEAD -->";
const BODY_MARKER: &str = "<!-- INJECT BODY -->";

// markers used in the rust template
const START_MARKER: &str = "--- START ---";
const RELOAD_MARKER: &str = "--- AUTORELOAD ---";
const MIDDLE_MARKER: &str = "--- MIDDLE ---";
const END_MARKER: &str = "--- END ---";

const HTML_HEAD_INSERT: &str = r##"
    <script type="module">import init from '/pkg/app.js';init('/pkg/app.wasm');</script>
    <link rel="preload" href="/pkg/app.wasm" as="fetch" type="application/wasm" crossorigin="">
    <link rel="stylesheet" href="/pkg/app.css">
    <link rel="modulepreload" href="/pkg/app.js">"##;

const HTML_HEAD_RELOAD: &str = r##"
    <script crossorigin="">(function () {
        var ws = new WebSocket('SOCKET_URL');
        ws.onmessage = (ev) => {
            console.log(`Reload message: ${ev.data}`);
            if (ev.data === 'reload') window.location.reload();
        };
        ws.onclose = () => console.warn('Autoreload stopped. Manual reload necessary.');
    })()
    </script>"##;

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
            HEAD_RE.find(&text).is_some(),
            format!("Missing Html marker {HEAD_MARKER}")
        );
        ensure!(
            BODY_RE.find(&text).is_some(),
            format!("Html missing marker {BODY_MARKER}")
        );
        Ok(Self { text })
    }

    fn autoreload(&self, config: &Config) -> String {
        HTML_HEAD_RELOAD.replace(
            "SOCKET_URL",
            &format!("ws://127.0.0.1:{}/autoreload", config.leptos.reload_port),
        )
    }

    fn head(&self, config: &Config) -> String {
        if config.watch {
            HTML_HEAD_INSERT.to_string() + &self.autoreload(config)
        } else {
            HTML_HEAD_INSERT.to_string()
        }
    }

    /// generate html for client side rendering
    pub fn generate_html(&self, config: &Config) -> Result<()> {
        fs::create_dir_all("target/site/").dot()?;
        let file = PathBuf::from("target/site/").with("index.html");

        let text = HEAD_RE.replace(&self.text, &self.head(config));
        let text = BODY_RE.replace(&text, "");

        if fs::write_if_changed(&file, &text.as_bytes())? {
            log::debug!("Html wrote html to {file:?}");
        } else {
            log::trace!("Html already up-to-date {file:?}");
        }
        Ok(())
    }

    /// generate rust for server side rendering
    pub fn generate_rust(&self, config: &Config) -> Result<()> {
        let file = &config.leptos.gen_file;

        let rust = include_str!("html_gen.rs");

        let head = HEAD_RE.find(&self.text).unwrap();
        let start = format!("{}{}", &self.text[0..head.start()].trim(), HTML_HEAD_INSERT);

        let body = BODY_RE.find(&self.text).unwrap();
        let middle = format!("  {}", &self.text[head.end()..body.start()].trim());

        let end = format!("  {}", &self.text[body.end()..].trim());

        let rust = START_RE.replacen(rust, 2, &start);
        let rust = RELOAD_RE.replace(&rust, &self.autoreload(config));
        let rust = MIDDLE_RE.replace(&rust, &middle);
        let rust = END_RE.replace(&rust, &end);

        if fs::write_if_changed(&file, &rust.as_bytes()).dot()? {
            log::debug!("Html wrote rust to {file}");
        } else {
            log::trace!("Html generated rust already up-to-date {file}");
        }
        Ok(())
    }
}
