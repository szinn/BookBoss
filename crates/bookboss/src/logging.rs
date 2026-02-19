use anyhow::{Context, Result};

pub fn init_logging() -> Result<()> {
    use tracing::subscriber::set_global_default;
    use tracing_log::LogTracer;
    use tracing_subscriber::{EnvFilter, Registry, fmt::format::FmtSpan, prelude::__tracing_subscriber_SubscriberExt};

    LogTracer::init_with_filter(log::LevelFilter::Off).context("Unable to setup log tracer")?;

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("sqlx::postgres::notice=off".parse()?)
        .add_directive("sqlx::query=off".parse()?)
        .add_directive("sea_orm=off".parse()?)
        .add_directive("dioxus_core=off".parse()?)
        .add_directive("dioxus_signals=off".parse()?)
        .add_directive("hyper::proto=off".parse()?)
        .add_directive("hyper::client=off".parse()?)
        .add_directive("h2=off".parse()?)
        .add_directive("rustls=off".parse()?)
        .add_directive("tokio_util=off".parse()?)
        .add_directive("tower_http=off".parse()?)
        .add_directive("reqwest=off".parse()?)
        .add_directive("simple_crypt=off".parse()?);

    let formatting_layer = tracing_subscriber::fmt::layer()
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_ansi(false);

    let subscriber = Registry::default().with(env_filter).with(formatting_layer);

    set_global_default(subscriber).context("Failed to set tracing subscriber xxx")?;

    Ok(())
}
