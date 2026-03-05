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
        Commands::DumpEpub { file } => {
            use bb_core::{book::FileFormat, pipeline::MetadataExtractor};
            use bb_formats::{EpubExtractor, read_opf_metadata_xml};

            let raw = read_opf_metadata_xml(&file)?;
            println!("=== raw OPF metadata ===\n{raw}\n");

            let meta = EpubExtractor.extract(&file, FileFormat::Epub).await?;
            println!("=== extracted metadata ===");
            println!("title:        {:?}", meta.title);
            println!("authors:      {:?}", meta.authors);
            println!("description:  {:?}", meta.description);
            println!("publisher:    {:?}", meta.publisher);
            println!("published:    {:?}", meta.published_date);
            println!("language:     {:?}", meta.language);
            println!("identifiers:  {:?}", meta.identifiers);
            println!("series_name:  {:?}", meta.series_name);
            println!("series_num:   {:?}", meta.series_number);
        }
        Commands::OpenLibrary { isbn } => {
            use bb_core::{
                book::IdentifierType,
                pipeline::{ExtractedIdentifier, ExtractedMetadata, MetadataProvider},
            };
            use bb_metadata::OpenLibraryAdapter;

            let isbn_type = if isbn.len() == 10 { IdentifierType::Isbn10 } else { IdentifierType::Isbn13 };
            let extracted = ExtractedMetadata {
                identifiers: Some(vec![ExtractedIdentifier {
                    identifier_type: isbn_type,
                    value: isbn.clone(),
                }]),
                ..Default::default()
            };

            let adapter = OpenLibraryAdapter::new();
            match adapter.enrich(&extracted).await? {
                None => println!("No record found on Open Library for ISBN {isbn}"),
                Some(book) => {
                    let m = &book.metadata;
                    println!("title:        {:?}", m.title);
                    println!("authors:      {:?}", m.authors);
                    println!("description:  {:?}", m.description);
                    println!("publisher:    {:?}", m.publisher);
                    println!("published:    {:?}", m.published_date);
                    println!("language:     {:?}", m.language);
                    println!("identifiers:  {:?}", m.identifiers);
                    println!("series_name:  {:?}", m.series_name);
                    println!("series_num:   {:?}", m.series_number);
                    println!("cover:        {}", if book.cover_bytes.is_some() { "found" } else { "not found" });
                }
            }
        }
        Commands::Server => {
            init_logging()?;
            let crate_version = clap::crate_version!();

            tracing::info!("BookBoss {}", crate_version);

            let span = tracing::span!(tracing::Level::TRACE, "CreateServer").entered();

            let database = open_database(&config.database).await.context("Couldn't create database connection")?;
            let repository_service = create_repository_service(database).await.context("Couldn't create database connection")?;
            let library_store = std::sync::Arc::new(bb_storage::LocalLibraryStore::new(config.library.library_path.clone()));
            let services = create_services(repository_service.clone(), library_store).context("Couldn't create core services")?;
            let frontend = launch_server_frontend(&config.frontend, services.clone());

            #[cfg(feature = "grpc")]
            let server = {
                use std::time::Duration;

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
