use clap::Parser;

#[derive(Clone, Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path for env file and mounting volume in the local file system
    #[arg(short = 'd', long, default_value = ".")]
    pub work_dir: String,

    /// Name of the env file which is to be written
    #[arg(short, long, default_value = ".flowsnet.env")]
    pub env_file: String,

    /// Flow identity in flows.network
    #[arg(short, long)]
    pub flow: String,

    /// Wasm file path in the local file system
    #[arg(short, long)]
    pub wasm: String,

    /// Port of the local service
    #[arg(short, long)]
    pub port: u16,
}
