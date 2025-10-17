use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{create_service, ServiceDefinitionFactory};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::Pattern;

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct PfSense;

impl ServiceDefinition for PfSense {
    fn name(&self) -> &'static str {
        "pfSense"
    }
    fn description(&self) -> &'static str {
        "Open-source firewall and router platform"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::NetworkSecurity
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::Endpoint(PortBase::Http, "/", "pfsense")
    }

    fn icon(&self) -> &'static str {
        "pfsense"
    }
}

inventory::submit!(ServiceDefinitionFactory::new(create_service::<PfSense>));
