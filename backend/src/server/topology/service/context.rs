use uuid::Uuid;

use crate::server::{
    groups::types::Group,
    hosts::types::{base::Host, interfaces::Interface, virtualization::HostVirtualization},
    services::types::{base::Service, virtualization::ServiceVirtualization},
    subnets::types::base::Subnet,
    topology::types::{
        api::TopologyRequestOptions,
        edges::Edge,
        nodes::{Node, NodeType},
    },
};

/// Central context for topology building operations
/// Reduces parameter passing and provides helper methods
pub struct TopologyContext<'a> {
    pub hosts: &'a [Host],
    pub subnets: &'a [Subnet],
    pub services: &'a [Service],
    pub groups: &'a [Group],
    pub options: &'a TopologyRequestOptions,
}

impl<'a> TopologyContext<'a> {
    pub fn new(
        hosts: &'a [Host],
        subnets: &'a [Subnet],
        services: &'a [Service],
        groups: &'a [Group],
        options: &'a TopologyRequestOptions,
    ) -> Self {
        Self {
            hosts,
            subnets,
            services,
            groups,
            options,
        }
    }

    pub fn get_subnet_by_id(&self, subnet_id: Uuid) -> Option<&'a Subnet> {
        self.subnets.iter().find(|s| s.id == subnet_id)
    }

    pub fn get_host_by_id(&self, host_id: Uuid) -> Option<&'a Host> {
        self.hosts.iter().find(|h| h.id == host_id)
    }

    pub fn get_service_by_id(&self, service_id: Uuid) -> Option<&'a Service> {
        self.services.iter().find(|s| s.id == service_id)
    }

    pub fn get_interface_by_id(&self, interface_id: Option<Uuid>) -> Option<&'a Interface> {
        self.hosts
            .iter()
            .find_map(|h| h.get_interface(&interface_id))
    }

    pub fn get_services_bound_to_interface(&self, interface_id: Uuid) -> Vec<&'a Service> {
        self.services
            .iter()
            .filter(|s| {
                s.to_bound_interface_ids()
                    .iter()
                    .any(|s| s.map(|id| id == interface_id).unwrap_or(false))
            })
            .collect()
    }

    pub fn get_node_subnet(&self, node_id: Uuid, nodes: &[Node]) -> Option<Uuid> {
        nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|node| match node.node_type {
                NodeType::InterfaceNode { subnet_id, .. } => subnet_id,
                NodeType::SubnetNode { .. } => node.id,
            })
    }

    pub fn edge_is_intra_subnet(&self, edge: &Edge) -> bool {
        if let (Some(source_subnet), Some(target_subnet)) = (
            self.get_subnet_from_interface_id(edge.source),
            self.get_subnet_from_interface_id(edge.target),
        ) {
            return source_subnet.id == target_subnet.id;
        }
        false
    }

    pub fn get_subnet_from_interface_id(&self, interface_id: Uuid) -> Option<&'a Subnet> {
        let interface = self
            .hosts
            .iter()
            .find_map(|h| h.base.interfaces.iter().find(|i| i.id == interface_id))?;

        self.get_subnet_by_id(interface.base.subnet_id)
    }

    pub fn get_host_from_interface_id(&self, interface_id: Uuid) -> Option<&'a Host> {
        self.hosts
            .iter()
            .find(|h| h.base.interfaces.iter().any(|i| i.id == interface_id))
    }

    pub fn get_host_is_virtualized_by(&self, host_id: &Uuid) -> Option<&Service> {
        if let Some(host) = self.get_host_by_id(*host_id)
            && let Some(HostVirtualization::Proxmox(proxmox_virtualization)) =
                &host.base.virtualization
        {
            return self
                .services
                .iter()
                .find(|s| s.id == proxmox_virtualization.service_id);
        }
        None
    }

    pub fn get_service_is_containerized_by(&self, service_id: &Uuid) -> Option<&Service> {
        if let Some(service) = self.get_service_by_id(*service_id)
            && let Some(ServiceVirtualization::Docker(docker_virtualization)) =
                &service.base.virtualization
        {
            return self
                .services
                .iter()
                .find(|s| s.id == docker_virtualization.service_id);
        }
        None
    }

    pub fn get_interfaces_with_infra_service(&self, subnet: &Subnet) -> Vec<Option<Uuid>> {
        self.services
            .iter()
            .filter(|s| {
                if let Some(host) = self.hosts.iter().find(|h| h.id == s.base.host_id) {
                    return (self
                        .options
                        .infra_service_categories
                        .contains(&s.base.service_definition.category())
                        || (self.options.show_gateway_as_infra_service && s.base.is_gateway))
                        && subnet.has_interface_with_service(host, s);
                }
                false
            })
            .flat_map(|s| s.to_bound_interface_ids())
            .collect()
    }

    pub fn is_interface_infra(&self, interface_id: Uuid) -> bool {
        if let Some(subnet) = self.get_subnet_from_interface_id(interface_id) {
            let infra_interfaces = self.get_interfaces_with_infra_service(subnet);
            return infra_interfaces.contains(&Some(interface_id));
        }
        false
    }

    /// Check if a subnet has both infra and non-infra nodes
    /// If it only has one type, infra constraints are not necessary
    pub fn subnet_has_mixed_infra(&self, subnet: &Subnet) -> bool {
        let infra_interfaces = self.get_interfaces_with_infra_service(subnet);

        // Get all interfaces in this subnet
        let all_interfaces_in_subnet: Vec<Uuid> = self
            .hosts
            .iter()
            .flat_map(|h| &h.base.interfaces)
            .filter(|i| i.base.subnet_id == subnet.id)
            .map(|i| i.id)
            .collect();

        if all_interfaces_in_subnet.is_empty() {
            return false;
        }

        // Check if we have both infra and non-infra
        let has_infra = all_interfaces_in_subnet
            .iter()
            .any(|id| infra_interfaces.contains(&Some(*id)));

        let has_non_infra = all_interfaces_in_subnet
            .iter()
            .any(|id| !infra_interfaces.contains(&Some(*id)));

        has_infra && has_non_infra
    }
}
