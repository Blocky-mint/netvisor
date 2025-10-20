use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{create_service, ServiceDefinitionFactory};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::Pattern;

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Unbound;

impl ServiceDefinition for Unbound {
    fn name(&self) -> &'static str {
        "Unbound DNS"
    }
    fn description(&self) -> &'static str {
        "Recursive DNS resolver with control interface"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::DNS
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![
            Pattern::Port(PortBase::DnsUdp),
            Pattern::Port(PortBase::new_tcp(8953)),
        ])
    }

    fn dashboard_icons_path(&self) -> &'static str {
        "unbound"
    }
}

inventory::submit!(ServiceDefinitionFactory::new(create_service::<Unbound>));
