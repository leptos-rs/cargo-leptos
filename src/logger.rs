//! TODO: port over formatting to `tracing-subscriber`
//! Currently, `tracing` emits log events

use ansi_term::{Colour::Fixed, Style};
use clap::builder::styling::{Ansi256Color, Color};
use std::io::Write;
use std::sync::OnceLock;

use crate::{config::Log, ext::StrAdditions};

const fn color(num: u8) -> Color {
    Color::Ansi256(Ansi256Color(num))
}

const ERR_RED: Color = color(196);
const WARN_YELLOW: Color = color(214);
const INFO_GREEN: Color = color(77);
const DBG_BLUE: Color = color(26);
const TRACE_VIOLET: Color = color(98);
pub const GRAY: Color = color(241);

static LOG_SELECT: OnceLock<LogFlag> = OnceLock::new();

pub fn setup(verbose: u8, logs: &[Log]) {
    let log_level = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    // OnceLock::get_or_try_init() is more idiomatic, but unstable at the moment
    _ = LOG_SELECT.get_or_init(|| {
        flexi_logger::Logger::try_with_str(log_level)
            .with_context(|| "Logger setup failed")
            .unwrap()
            .filter(Box::new(Filter))
            .format(format)
            .start()
            .expect("Couldn't init cargo-leptos logger");

        LogFlag::new(logs)
    });
}

#[derive(Debug, Clone, Copy)]
struct LogFlag(u8);

impl LogFlag {
    fn new(logs: &[Log]) -> Self {
        Self(logs.iter().fold(0, |acc, f| acc | f.flag()))
    }

    fn is_set(&self, log: Log) -> bool {
        log.flag() & self.0 != 0
    }

    fn matches(&self, target: &str) -> bool {
        self.do_server_log(target) || self.do_wasm_log(target)
    }

    fn do_server_log(&self, target: &str) -> bool {
        self.is_set(Log::Server) && (target.starts_with("hyper") || target.starts_with("axum"))
    }

    fn do_wasm_log(&self, target: &str) -> bool {
        self.is_set(Log::Wasm) && (target.starts_with("wasm") || target.starts_with("walrus"))
    }
}

impl Log {
    fn flag(&self) -> u8 {
        match self {
            Self::Wasm => 0b0000_0001,
            Self::Server => 0b0000_0010,
        }
    }
}

// https://docs.rs/flexi_logger/0.24.1/flexi_logger/type.FormatFunction.html
fn format(
    write: &mut dyn Write,
    _now: &mut DeferredNow,
    record: &Record<'_>,
) -> Result<(), std::io::Error> {
    let args = record.args().to_string();

    let lvl_color = record.level().color();

    if let Some(dep) = dependency(record) {
        let dep = format!("[{}]", dep);
        let dep = dep.pad_left_to(12);
        write!(write, "{} {}", lvl_color.paint(dep), record.args())
    } else {
        let (word, rest) = split(&args);
        let word = word.pad_left_to(12);
        write!(write, "{} {rest}", lvl_color.paint(word))
    }
}

fn split(args: &str) -> (&str, &str) {
    match args.find(' ') {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => ("", args),
    }
}
fn dependency<'a>(record: &'a Record<'_>) -> Option<&'a str> {
    let target = record.target();

    if !target.starts_with("cargo_leptos") {
        if let Some((ent, _)) = target.split_once("::") {
            return Some(ent);
        }
    }
    None
}

pub struct Filter;
impl LogLineFilter for Filter {
    fn write(
        &self,
        now: &mut DeferredNow,
        record: &Record,
        log_line_writer: &dyn LogLineWriter,
    ) -> std::io::Result<()> {
        let target = record.target();
        if record.level() == Level::Error
            || target.starts_with("cargo_leptos")
            // LOG_SELECT will have been initialized by now, get_or_init() not required
            || LOG_SELECT.get().is_some_and(|flag| flag.matches(target))
        {
            log_line_writer.write(now, record)?;
        }
        Ok(())
    }
}

trait LevelExt {
    fn color(&self) -> Color;
}

impl LevelExt for Level {
    fn color(&self) -> Color {
        match self {
            Level::Error => ERR_RED,
            Level::Warn => WARN_YELLOW,
            Level::Info => INFO_GREEN,
            Level::Debug => DBG_BLUE,
            Level::Trace => TRACE_VIOLET,
        }
    }
}
