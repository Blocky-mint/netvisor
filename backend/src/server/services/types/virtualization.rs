use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum_macros::IntoStaticStr;
use uuid::Uuid;
use validator::Validate;

use crate::server::shared::{
    constants::Entity,
    types::metadata::{EntityMetadataProvider, HasId, TypeMetadataProvider},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "type", content = "details")]
pub enum ServiceVirtualization {
    Docker(DockerVirtualization),
}

#[derive(Debug, Clone, Serialize, Validate, Deserialize, PartialEq, Eq, Hash)]
pub struct DockerVirtualization {
    pub container_name: Option<String>,
    pub container_id: Option<String>,
    pub service_id: Uuid,
}

impl HasId for ServiceVirtualization {
    fn id(&self) -> &'static str {
        self.into()
    }
}

impl EntityMetadataProvider for ServiceVirtualization {
    fn color(&self) -> &'static str {
        Entity::Virtualization.color()
    }
    fn icon(&self) -> &'static str {
        Entity::Virtualization.icon()
    }
}

impl TypeMetadataProvider for ServiceVirtualization {
    fn name(&self) -> &'static str {
        "Docker"
    }

    fn description(&self) -> &'static str {
        "A service running in a docker container"
    }
}
