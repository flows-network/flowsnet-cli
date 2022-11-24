mod cli;
mod executor;

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

const TIMEOUT: u64 = 30;
const SERVER_HOST: &str = "http://127.0.0.1:9090";

lazy_static! {
    static ref HEART_INTERVAL: Duration = Duration::from_secs(30);
    pub static ref HTTP_CLIENT: Client = ClientBuilder::new()
        .timeout(Duration::from_secs(TIMEOUT))
        .build()
        .expect("Can't build the reqwest client");
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let (shutdown_tx, shutdown_rx) = broadcast::channel::<bool>(1);
    let rx = shutdown_tx.subscribe();
    let rx2 = shutdown_tx.subscribe();
    let args2 = args.clone();
    tokio::spawn(async {
        executor::start(args2, rx).await;
    });

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

    run_proxy(&args, shutdown_rx).await;

    heart(&args.flow, rx2).await;
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

        _ = rathole::run(args, shutdown_rx).await;
    });
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
        .post(format!("{}/link/{}", SERVER_HOST, flow))
        .send()
        .await;
    match response {
        Ok(r) => r.json::<LinkResult>().await.map_err(|e| e.into()),
        Err(e) => Err(e.into()),
    }
}

async fn heart(flow: &str, mut shutdown_rx: broadcast::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(*HEART_INTERVAL) => {
                _ = HTTP_CLIENT
                    .post(format!("{}/heart/{}", SERVER_HOST, flow))
                    .send()
                    .await;
            }
            _ = shutdown_rx.recv() => break
        }
    }
}
