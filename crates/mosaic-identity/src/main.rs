//! Mosaic Identity Service — binary entry point.
//!
//! HTTP server for key management, signing, and identity binding.
//! Run with: cargo run -p mosaic-identity -- --database mosaic-identity.db

use std::net::SocketAddr;

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use mosaic_identity::{api, storage::Storage};

#[derive(Parser)]
#[command(name = "mosaic-identity", version = "0.1.0")]
struct Cli {
    /// SQLite database URL (sqlite:///path/to/db)
    #[arg(
        short,
        long,
        env = "MIS_DATABASE_URL",
        default_value = "sqlite:mosaic-identity.db?mode=rwc"
    )]
    database: String,

    /// Listen host
    #[arg(short = 'H', long, env = "MIS_HOST", default_value = "0.0.0.0")]
    host: String,

    /// Listen port
    #[arg(short, long, env = "MIS_PORT", default_value_t = 8081)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

    tracing::info!("Opening database: {}", cli.database);
    let storage = Storage::open(&cli.database).await?;

    let app = api::router(storage);

    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;
    tracing::info!("Mosaic Identity Service starting on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
