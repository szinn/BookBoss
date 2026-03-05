#[derive(Debug, clap::Parser)]
#[command(
    name = "BookBoss",
    help_template = r#"
{before-help}{name} {version} - {about}

{usage-heading} {usage}

{all-args}{after-help}

AUTHORS:
    {author}
"#,
    version,
    author
)]
#[command(about, long_about = None)]
#[command(propagate_version = true, arg_required_else_help = true)]
pub struct CommandLine {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, clap::Subcommand)]
pub enum GrpcSubcommand {
    #[command(about = "Query the status of the running server")]
    Status,
}

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    #[command(about = "Start server", display_order = 10)]
    Server,
    #[command(about = "Dump metadata extracted from an EPUB file", display_order = 20)]
    DumpEpub {
        #[arg(help = "Path to the EPUB file")]
        file: std::path::PathBuf,
    },
    #[command(about = "Look up a book by ISBN on Open Library and dump the result", display_order = 30)]
    OpenLibrary {
        #[arg(help = "ISBN-10 or ISBN-13")]
        isbn: String,
    },
    #[command(about = "Look up a book by ISBN on Hardcover and dump the result", display_order = 40)]
    Hardcover {
        #[arg(help = "ISBN-10 or ISBN-13")]
        isbn: String,
    },
    #[command(about = "Interact with a running BookBoss server via gRPC", display_order = 50)]
    Grpc {
        #[arg(short = 'H', long, default_value = "localhost", help = "Host to connect to")]
        host: String,
        #[arg(short = 'p', long, default_value_t = 8081, help = "gRPC port")]
        port: u16,
        #[command(subcommand)]
        command: GrpcSubcommand,
    },
}
