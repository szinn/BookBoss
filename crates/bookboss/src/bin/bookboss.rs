use std::time::Duration;

#[cfg(not(feature = "server"))]
fn main() {
    bb_frontend::web::launch_web_frontend();
}

#[cfg(feature = "server")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use bb_core::create_services;
    use bb_database::{create_repository_service, open_database};
    use bb_frontend::server::launch_server_frontend;
    use bookboss::{
        commands::{CommandLine, Commands},
        config::Config,
        logging::init_logging,
    };
    #[cfg(feature = "grpc")]
    use {
        bb_api::create_api_subsystem,
        tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle, Toplevel},
    };

    let cli: CommandLine = clap::Parser::parse();
    let config = Config::load().context("Cannot load configuration")?;

    match cli.command {
        Commands::Server => {
            init_logging()?;
            let crate_version = clap::crate_version!();

            tracing::info!("BookBoss {}", crate_version);

            let span = tracing::span!(tracing::Level::TRACE, "CreateServer").entered();

            let database = open_database(&config.database).await.context("Couldn't create database connection")?;
            let repository_service = create_repository_service(database).await.context("Couldn't create database connection")?;
            let services = create_services(repository_service.clone()).context("Couldn't create core services")?;
            let frontend = launch_server_frontend(&config.frontend, services.clone());

            #[cfg(feature = "grpc")]
            let server = {
                let api_subsystem = create_api_subsystem(&config.api, services.clone());

                Toplevel::new(async |s: &mut SubsystemHandle| {
                    s.start(SubsystemBuilder::new("Api", api_subsystem.into_subsystem()));
                })
                .catch_signals()
                .handle_shutdown_requests(Duration::from_millis(1000))
            };

            span.exit();

            // Wait for shutdown request
            #[cfg(feature = "grpc")]
            server.await?;
            let _ = frontend.join();

            repository_service.repository().close().await.context("Couldn't close database")?;
        }
    }

    Ok(())
}
