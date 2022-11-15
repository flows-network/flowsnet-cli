use anyhow::Result;
use std::path::PathBuf;
use tokio::{signal, sync::broadcast};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = broadcast::channel::<bool>(1);
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            // Something really weird happened. So just panic
            panic!("Failed to listen for the ctrl-c signal: {:?}", e);
        }

        if let Err(e) = shutdown_tx.send(true) {
            // shutdown signal must be catched and handle properly
            // `rx` must not be dropped
            panic!("Failed to send shutdown signal: {:?}", e);
        }
    });

    {
        let is_atty = atty::is(atty::Stream::Stdout);

        let level = "info"; // if RUST_LOG not present, use `info` level
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from(level)),
            )
            .with_ansi(is_atty)
            .init();
    }

    tokio::spawn(async {
        let args = rathole::Cli {
            config_path: Some(PathBuf::from("./target/debug/client.toml")),
            server: false,
            client: true,
            genkey: None,
        };

        rathole::run(args, shutdown_rx).await
    });

    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(())
}
