use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{ServiceDefinitionFactory, create_service};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::Pattern;

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct PowerDNS;

impl ServiceDefinition for PowerDNS {
    fn name(&self) -> &'static str {
        "PowerDNS"
    }
    fn description(&self) -> &'static str {
        "Authoritative DNS server with API"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::DNS
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![
            Pattern::Port(PortBase::DnsUdp),
            Pattern::Port(PortBase::DnsTcp),
            Pattern::Port(PortBase::new_tcp(8081)),
        ])
    }

    fn dashboard_icons_path(&self) -> &'static str {
        "powerdns"
    }
}

inventory::submit!(ServiceDefinitionFactory::new(create_service::<PowerDNS>));
