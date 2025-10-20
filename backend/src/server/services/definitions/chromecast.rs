use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{create_service, ServiceDefinitionFactory};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::{Pattern, Vendor};

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct ChromecastDevice;

impl ServiceDefinition for ChromecastDevice {
    fn name(&self) -> &'static str {
        "Chromecast"
    }

    fn description(&self) -> &'static str {
        "Google Chromecast streaming device"
    }

    fn category(&self) -> ServiceCategory {
        ServiceCategory::IoT
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![
            Pattern::MacVendor(Vendor::GOOGLE),
            Pattern::Port(PortBase::new_tcp(8008)),
            Pattern::Port(PortBase::new_tcp(8009)),
        ])
    }

    fn simple_icons_path(&self) -> &'static str {
        "googlecast"
    }
}

inventory::submit!(ServiceDefinitionFactory::new(
    create_service::<ChromecastDevice>
));
