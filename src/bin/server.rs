use axum::{
    http::{Method},
    Router,
};
use clap::Parser;
use netvisor::server::{config::ServerConfig, discovery::manager::DiscoverySessionManager, shared::{handlers::create_router, types::storage::StorageFactory}};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "netvisor-server")]
#[command(about = "NetVisor network diagnostics server")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "netvisor.toml")]
    config: String,
    
    /// Override server host
    #[arg(long)]
    host: Option<String>,
    
    /// Override server port
    #[arg(long)]
    port: Option<u16>,
    
    /// Override log level
    #[arg(long)]
    log_level: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv::dotenv();
    
    let cli = Cli::parse();
    
    // Load configuration
    let mut config = ServerConfig::load()?;
    
    // Apply CLI overrides
    if let Some(host) = cli.host {
        config.server.host = host;
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }
    if let Some(log_level) = cli.log_level {
        config.server.log_level = log_level;
    }
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("debug"))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    // Initialize storage
    let storage = StorageFactory::new_sqlite(&config.database_url()).await?;
    
    // Create app state
    let state = Arc::new(netvisor::server::config::AppState {
        config: config.clone(),
        node_storage: storage.nodes,
        node_group_storage: storage.node_groups,
        diagnostic_storage: storage.diagnostics,
        daemon_storage: storage.daemons,
        discovery_manager: DiscoverySessionManager::new()
    });

    // Create discovery cleanup task
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            
            // Check for timeouts (fail sessions running > 10 minutes)
            // cleanup_state.discovery_manager.check_timeouts(10).await;
            
            // Clean up old sessions (remove completed sessions > 24 hours old)
            cleanup_state.discovery_manager.cleanup_old_sessions(24).await;
        }
    });
    
    // Create router
    let api_router = create_router().with_state(state);
    
    // Create main app
    let app = Router::new()
        .merge(api_router)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                        .allow_headers(Any),
                ),
        );
    
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    tracing::info!("🚀 NetVisor server starting on http://{}", addr);
    tracing::info!("📊 Web UI available at http://{}", addr);
    tracing::info!("🔧 API available at http://{}/api", addr);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}