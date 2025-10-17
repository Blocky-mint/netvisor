use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{create_service, ServiceDefinitionFactory};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::{Pattern, Vendor};

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct UnifiAccessPoint;

impl ServiceDefinition for UnifiAccessPoint {
    fn name(&self) -> &'static str {
        "Unifi Access Point"
    }
    fn description(&self) -> &'static str {
        "Ubiquiti UniFi wireless access point"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::NetworkAccess
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![
            Pattern::MacVendor(Vendor::UBIQUITI),
            Pattern::Endpoint(PortBase::Http, "/", "Unifi"),
        ])
    }

    fn icon(&self) -> &'static str {
        "unifi"
    }
}

inventory::submit!(ServiceDefinitionFactory::new(
    create_service::<UnifiAccessPoint>
));
