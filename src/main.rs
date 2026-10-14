use mini_bot_rs::agent;
use mini_bot_rs::config::Config;
use mini_bot_rs::gateway;
use mini_bot_rs::i18n::{tr, tr_with_args};

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
    /// 查看有效設定，或寫出範例設定檔
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// 查看有效設定。不顯示 API key 內容，只顯示來源。
    Show,
    /// 寫出不含金鑰的範例設定。省略 --output 時印到標準輸出。
    Example {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = fmt::Subscriber::builder()
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    let cli = Cli::parse();
    let config_path = match cli.config_dir {
        Some(dir) => PathBuf::from(dir).join("config.toml"),
        None => Config::default_path(),
    };

    match cli.command {
        Commands::Agent { message } => {
            info!("{}", tr("app.starting_agent"));
            agent::run(message, &config_path).await?;
        }
        Commands::Gateway { port, host } => {
            let port = port.unwrap_or(3000);
            let host = host.unwrap_or_else(|| "127.0.0.1".to_string());
            info!(
                "{}",
                tr_with_args("app.starting_gateway", &[&host, &port.to_string()])
            );
            gateway::run(&host, port, &config_path).await?;
        }
        Commands::Version => {
            let version = env!("CARGO_PKG_VERSION");
            println!("{}", tr_with_args("app.version", &[&version]));
        }
        Commands::Config { action } => match action {
            None | Some(ConfigAction::Show) => {
                println!("{}", Config::show(&config_path)?);
            }
            Some(ConfigAction::Example { output }) => match output {
                Some(path) => {
                    Config::write_example(&path)?;
                    println!("已寫入 {}", path.display());
                }
                None => print!("{}", Config::example_toml()?),
            },
        },
    }

    Ok(())
}
