use chrono::Local;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use super::main::LoggerProvider;

pub struct TracingEngine;

struct CustomTimeFormat;

impl FormatTime for CustomTimeFormat {
    fn format_time(
        &self,
        writer: &mut tracing_subscriber::fmt::format::Writer<'_>,
    ) -> std::fmt::Result {
        write!(writer, "{}", Local::now().format("%Y-%m-%d %H:%M:%S"))
    }
}
impl LoggerProvider for TracingEngine {
    fn init(&self) {
        let _ = LogTracer::init();

        setup_tracing();
    }
}

fn setup_tracing() {
    let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());

    let log_dir = "./logs";
    std::fs::create_dir_all(log_dir).expect("Failed to create log directory");
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        Path::new(log_dir),
        "current.log",
    );
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let combined_writer = stdout_writer.and(file_writer);

    let custom_format = fmt::format()
        .with_target(true)
        .with_timer(CustomTimeFormat)
        .with_ansi(false)
        .with_source_location(false)
        .with_level(true);

    let result = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            fmt::layer()
                .event_format(custom_format)
                .with_writer(combined_writer),
        )
        .try_init();

    if let Err(err) = result {
        eprintln!("Warning: Failed to set global subscriber (already initialized): {err}");
    }

    Box::leak(Box::new(stdout_guard));
    Box::leak(Box::new(file_guard));
}