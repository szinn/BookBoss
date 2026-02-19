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
pub enum Commands {
    #[command(about = "Start server", display_order = 10)]
    Server,
}
