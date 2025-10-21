use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{create_service, ServiceDefinitionFactory};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::Pattern;

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct HpPrinter;

impl ServiceDefinition for HpPrinter {
    fn name(&self) -> &'static str {
        "Hp Printer"
    }
    fn description(&self) -> &'static str {
        "An HP Printer"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::Printer
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![
            Pattern::AnyOf(vec![
                Pattern::Endpoint(PortBase::Http, "", "LaserJet"),
                Pattern::Endpoint(PortBase::Http, "", "DeskJet"),
                Pattern::Endpoint(PortBase::Http, "", "OfficeJet"),
            ]),
            Pattern::AnyOf(vec![
                Pattern::Port(PortBase::Ipp),
                Pattern::Port(PortBase::LdpTcp),
                Pattern::Port(PortBase::LdpUdp),
            ]),
        ])
    }

    fn dashboard_icons_path(&self) -> &'static str {
        "hp"
    }

    fn logo_needs_white_background(&self) -> bool {
        true
    }
}

inventory::submit!(ServiceDefinitionFactory::new(create_service::<HpPrinter>));
