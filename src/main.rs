mod cli;

use clap::Parser;
use cli::Cli;
use lazy_static::lazy_static;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::{signal, sync::broadcast};
use tracing_subscriber::EnvFilter;

const TIMEOUT: u64 = 120;

lazy_static! {
    pub static ref HTTP_CLIENT: Client = ClientBuilder::new()
        .timeout(Duration::from_secs(TIMEOUT))
        .build()
        .expect("Can't build the reqwest client");
}

#[tokio::main]
async fn main() {
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
    let args = Cli::parse();
    run_proxy(&args, shutdown_rx).await;
}

async fn run_proxy(args: &Cli, shutdown_rx: broadcast::Receiver<bool>) {
    let link_result = link(&args.flow).await.unwrap();

    let config = Config {
        client: ConfigClient {
            remote_addr: link_result.remote_addr,
            services: HashMap::from([(
                args.flow.clone(),
                Service {
                    token: link_result.token,
                    local_addr: format!("127.0.0.1:{}", args.port),
                },
            )]),
        },
    };
    let config = toml::to_string(&config).unwrap();

    let mut config_path = PathBuf::from(std::env::temp_dir());
    config_path.push("flowsnet-client.toml");

    std::fs::write(config_path.clone(), config).unwrap();

    tokio::spawn(async {
        let args = rathole::Cli {
            config_path: Some(config_path),
            server: false,
            client: true,
            genkey: None,
        };

        rathole::run(args, shutdown_rx).await
    });

    std::thread::sleep(std::time::Duration::from_secs(5));
}

#[derive(Serialize, Deserialize)]
struct Config {
    client: ConfigClient,
}
#[derive(Serialize, Deserialize)]
struct ConfigClient {
    remote_addr: String,
    services: HashMap<String, Service>,
}
#[derive(Serialize, Deserialize)]
struct Service {
    token: String,
    local_addr: String,
}

#[derive(Deserialize)]
struct LinkResult {
    token: String,
    remote_addr: String,
}

async fn link(flow: &str) -> anyhow::Result<LinkResult> {
    let response = HTTP_CLIENT
        .get(format!("http://127.0.0.1:9090/link/{}", flow))
        .send()
        .await;
    match response {
        Ok(r) => r.json::<LinkResult>().await.map_err(|e| e.into()),
        Err(e) => Err(e.into()),
    }
}
