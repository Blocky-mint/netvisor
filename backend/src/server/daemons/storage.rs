use std::net::IpAddr;

use crate::server::daemons::types::base::{Daemon, DaemonBase};
use anyhow::Error;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

#[async_trait]
pub trait DaemonStorage: Send + Sync {
    async fn create(&self, daemon: &Daemon) -> Result<()>;
    async fn get_by_id(&self, id: &Uuid) -> Result<Option<Daemon>>;
    async fn get_by_host_id(&self, host_id: &Uuid) -> Result<Option<Daemon>>;
    async fn get_all(&self, network_id: &Uuid) -> Result<Vec<Daemon>>;
    async fn update(&self, group: &Daemon) -> Result<()>;
    async fn delete(&self, id: &Uuid) -> Result<()>;
}

pub struct PostgresDaemonStorage {
    pool: PgPool,
}

impl PostgresDaemonStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DaemonStorage for PostgresDaemonStorage {
    async fn create(&self, daemon: &Daemon) -> Result<()> {
        let ip_str = serde_json::to_string(&daemon.base.ip)?;

        sqlx::query(
            r#"
            INSERT INTO daemons (
                id, host_id, ip, port,
                last_seen, registered_at, network_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(daemon.id)
        .bind(daemon.base.host_id)
        .bind(ip_str)
        .bind(Into::<i32>::into(daemon.base.port))
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .bind(daemon.base.network_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_by_id(&self, id: &Uuid) -> Result<Option<Daemon>> {
        let row = sqlx::query("SELECT * FROM daemons WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_daemon(row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_host_id(&self, host_id: &Uuid) -> Result<Option<Daemon>> {
        let row = sqlx::query("SELECT * FROM daemons WHERE host_id = $1")
            .bind(host_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_daemon(row)?)),
            None => Ok(None),
        }
    }

    async fn get_all(&self, network_id: &Uuid) -> Result<Vec<Daemon>> {
        let rows = sqlx::query("SELECT * FROM daemons WHERE network_id = $1")
            .bind(network_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                info!("SQLx error in get_all: {:?}", e);
                e
            })?;

        let mut daemons = Vec::new();
        for row in rows {
            daemons.push(row_to_daemon(row)?);
        }

        Ok(daemons)
    }

    async fn update(&self, daemon: &Daemon) -> Result<()> {
        let ip_str = serde_json::to_string(&daemon.base.ip)?;

        sqlx::query(
            r#"
            UPDATE daemons SET 
                host_id = $2, ip = $3, port = $4, last_seen = $5
            WHERE id = $1
            "#,
        )
        .bind(daemon.id)
        .bind(daemon.base.host_id)
        .bind(ip_str)
        .bind(daemon.base.port as i32)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &Uuid) -> Result<()> {
        sqlx::query("DELETE FROM daemons WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

fn row_to_daemon(row: sqlx::postgres::PgRow) -> Result<Daemon, Error> {
    let ip: IpAddr = serde_json::from_str(&row.get::<String, _>("ip"))
        .or(Err(Error::msg("Failed to deserialize IP")))?;

    Ok(Daemon {
        id: row.get("id"),
        last_seen: row.get("last_seen"),
        registered_at: row.get("registered_at"),
        base: DaemonBase {
            ip,
            port: row.get::<i32, _>("port").try_into().unwrap(),
            host_id: row.get("host_id"),
            network_id: row.get("network_id"),
        },
    })
}
