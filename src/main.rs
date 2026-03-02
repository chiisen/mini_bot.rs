mod agent;
mod config;
mod gateway;
mod memory;
mod providers;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "mini_bot.rs")]
#[command(about = "MiniBot MVP - A minimal Rust AI Agent runtime", long_about = None)]
struct Cli {
    #[arg(long)]
    config_dir: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Agent {
        #[arg(short, long)]
        message: Option<String>,
    },
    Gateway {
        #[arg(short, long)]
        port: Option<u16>,
        #[arg(long)]
        host: Option<String>,
    },
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = fmt::Subscriber::builder()
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    let cli = Cli::parse();

    match cli.command {
        Commands::Agent { message } => {
            info!("Starting MiniBot Agent...");
            agent::run(message).await?;
        }
        Commands::Gateway { port, host } => {
            let port = port.unwrap_or(3000);
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            info!("Starting Gateway at {}:{}", host, port);
            gateway::run(&host, port).await?;
        }
        Commands::Version => {
            println!("MiniBot MVP v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
