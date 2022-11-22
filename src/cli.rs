use clap::Parser;

#[derive(Clone, Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Flow identity in flows.network
    #[arg(short, long)]
    pub flow: String,

    /// Wasm path in the local file system
    #[arg(short, long)]
    pub wasm: String,

    /// Port of the local service
    #[arg(short, long)]
    pub port: u16,
}
