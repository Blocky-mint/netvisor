use crate::server::services::definitions::{create_service, ServiceDefinitionFactory};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::{Pattern, Vendor};

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct EeroRepeater;

impl ServiceDefinition for EeroRepeater {
    fn name(&self) -> &'static str {
        "Eero Repeater"
    }
    fn description(&self) -> &'static str {
        "Eero device providing mesh network services"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::NetworkAccess
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![Pattern::MacVendor(Vendor::EERO), Pattern::IsGateway])
    }
}

inventory::submit!(ServiceDefinitionFactory::new(
    create_service::<EeroRepeater>
));
