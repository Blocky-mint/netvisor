use anyhow::{Error, Result};
use figment::{
    providers::{Env, Serialized},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

use crate::server::{shared::{services::ServiceFactory, types::storage::StorageFactory}, users::types::{User, UserBase}};
use crate::server::{discovery::manager::DiscoverySessionManager, utils::base::ServerNetworkUtils};

/// CLI arguments structure (for figment integration)
#[derive(Debug)]
pub struct CliArgs {
    pub server_port: Option<u16>,
    pub log_level: Option<String>,
    pub rust_log: Option<String>,
    pub database_url: Option<String>,
    pub web_external_path: Option<String>,
}

/// Flattened server configuration struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    // Server settings
    /// What port the server should listen on
    pub server_port: u16,

    /// Level of logs to show
    pub log_level: String,

    /// Rust log level
    pub rust_log: String,

    /// Where database should be located
    pub database_url: String,

    /// Where static web assets are located for serving
    pub web_external_path: Option<PathBuf>,

    /// Whether to seed a test user, used for headless integration testing
    pub seed_test_user: bool
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_port: 60072,
            log_level: "info".to_string(),
            rust_log: "".to_string(),
            database_url: "postgresql://postgres:password@localhost:5432/netvisor".to_string(),
            web_external_path: None,
            seed_test_user: false,
        }
    }
}

impl ServerConfig {
    pub fn load(cli_args: CliArgs) -> anyhow::Result<Self> {
        // Standard configuration layering: Defaults → Env → CLI (highest priority)
        let mut figment = Figment::from(Serialized::defaults(ServerConfig::default()));

        // Add environment variables with NETVISOR_ prefix
        figment = figment.merge(Env::prefixed("NETVISOR_"));

        // Add CLI overrides (highest priority) - only if explicitly provided
        if let Some(server_port) = cli_args.server_port {
            figment = figment.merge(("server_port", server_port));
        }
        if let Some(log_level) = cli_args.log_level {
            figment = figment.merge(("log_level", log_level));
        }
        if let Some(rust_log) = cli_args.rust_log {
            figment = figment.merge(("rust_log", rust_log));
        }
        if let Some(database_url) = cli_args.database_url {
            figment = figment.merge(("database_url", database_url));
        }
        if let Some(web_external_path) = cli_args.web_external_path {
            figment = figment.merge(("web_external_path", web_external_path));
        }

        let config: ServerConfig = figment
            .extract()
            .map_err(|e| Error::msg(format!("Configuration error: {}", e)))?;

        Ok(config)
    }

    pub fn database_url(&self) -> String {
        self.database_url.to_string()
    }
}

pub struct AppState {
    pub config: ServerConfig,
    pub storage: StorageFactory,
    pub services: ServiceFactory,
    pub discovery_manager: DiscoverySessionManager,
    pub utils: ServerNetworkUtils,
}

impl AppState {
    pub async fn new(
        config: ServerConfig,
        discovery_manager: DiscoverySessionManager,
        utils: ServerNetworkUtils,
    ) -> Result<Arc<Self>, Error> {
        let storage = StorageFactory::new(&config.database_url()).await?;
        let services = ServiceFactory::new(&storage).await?;

        if config.seed_test_user {
            services.user_service.create_user(User::new(UserBase::default())).await?;
        }

        Ok(Arc::new(Self {
            config,
            storage,
            services,
            discovery_manager,
            utils,
        }))
    }
}
