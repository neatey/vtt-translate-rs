use anyhow::Result;
use clap::Parser;
use vtt_translate::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    vtt_translate::run(args).await
}
