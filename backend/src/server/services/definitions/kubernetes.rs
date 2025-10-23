use crate::server::hosts::types::ports::PortBase;
use crate::server::services::definitions::{ServiceDefinitionFactory, create_service};
use crate::server::services::types::categories::ServiceCategory;
use crate::server::services::types::definitions::ServiceDefinition;
use crate::server::services::types::patterns::Pattern;

#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Kubernetes;

impl ServiceDefinition for Kubernetes {
    fn name(&self) -> &'static str {
        "Kubernetes"
    }
    fn description(&self) -> &'static str {
        "Container orchestration platform"
    }
    fn category(&self) -> ServiceCategory {
        ServiceCategory::Virtualization
    }

    fn discovery_pattern(&self) -> Pattern<'_> {
        Pattern::AllOf(vec![
            Pattern::Port(PortBase::new_tcp(6443)),
            Pattern::AnyOf(vec![
                Pattern::Port(PortBase::new_tcp(10250)),
                Pattern::Port(PortBase::new_tcp(10259)),
                Pattern::Port(PortBase::new_tcp(10257)),
                Pattern::Port(PortBase::new_tcp(10256)),
            ]),
        ])
    }

    fn dashboard_icons_path(&self) -> &'static str {
        "kubernetes"
    }
}

inventory::submit!(ServiceDefinitionFactory::new(create_service::<Kubernetes>));
