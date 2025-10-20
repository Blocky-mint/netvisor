use crate::daemon::discovery::service::base::{
    CreatesDiscoveredEntities, DiscoversNetworkedEntities, Discovery, HasDiscoveryType,
    SCAN_TIMEOUT,
};
use crate::daemon::discovery::types::base::{DiscoveryCriticalError, DiscoverySessionUpdate};
use crate::server::discovery::types::base::DiscoveryType;
use crate::server::hosts::types::ports::TransportProtocol;
use crate::server::hosts::types::{
    interfaces::{Interface, InterfaceBase},
    ports::PortBase,
};
use crate::server::services::types::base::{Service, ServiceMatchBaselineParams};
use crate::server::services::types::endpoints::Endpoint;
use crate::{
    daemon::utils::base::DaemonUtils,
    server::{
        daemons::types::api::DaemonDiscoveryRequest,
        hosts::types::base::Host,
        services::types::endpoints::EndpointResponse,
        subnets::types::base::{Subnet, SubnetType},
    },
};
use anyhow::anyhow;
use anyhow::{Error, Result};
use axum::async_trait;
use cidr::IpCidr;
use dhcproto::v4::{self, Decodable, Encodable, Encoder, Message, MessageType};
use futures::{
    future::try_join_all,
    stream::{self, StreamExt},
};
use rand::{Rng, SeedableRng};
use rsntp::AsyncSntpClient;
use snmp2::{AsyncSession, Oid};
use std::net::SocketAddr;
use std::result::Result::Ok;
use std::time::Duration;
use std::{net::IpAddr, sync::Arc};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use trust_dns_resolver::config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts};
use trust_dns_resolver::TokioAsyncResolver;

#[derive(Default)]
pub struct NetworkScanDiscovery {}

impl HasDiscoveryType for Discovery<NetworkScanDiscovery> {
    fn discovery_type(&self) -> DiscoveryType {
        DiscoveryType::Network
    }
}

impl CreatesDiscoveredEntities for Discovery<NetworkScanDiscovery> {}

#[async_trait]
impl DiscoversNetworkedEntities for Discovery<NetworkScanDiscovery> {
    async fn start_discovery_session(
        &self,
        request: DaemonDiscoveryRequest,
        cancel: CancellationToken,
    ) -> Result<(), Error> {
        let subnets = self.discover_create_subnets().await?;

        let total_ips_across_subnets: usize = subnets
            .iter()
            .map(|subnet| subnet.base.cidr.iter().count())
            .sum();

        self.start_discovery(total_ips_across_subnets, request)
            .await?;

        let discovery_futures = subnets
            .iter()
            .map(|subnet| self.scan_and_process_hosts(subnet, cancel.clone()));

        let discovery_result = try_join_all(discovery_futures).await.map(|_| ());

        self.finish_discovery(discovery_result, cancel.clone())
            .await?;

        Ok(())
    }

    async fn get_gateway_ips(&self) -> Result<Vec<IpAddr>, Error> {
        self.as_ref().utils.get_routing_table_gateway_ips().await
    }

    async fn discover_create_subnets(&self) -> Result<Vec<Subnet>, Error> {
        let daemon_id = self.as_ref().config_store.get_id().await?;
        let (_, subnets) = self
            .as_ref()
            .utils
            .scan_interfaces(self.discovery_type(), daemon_id)
            .await?;
        let subnet_futures = subnets.iter().map(|subnet| self.create_subnet(subnet));
        let subnets = try_join_all(subnet_futures).await?;

        Ok(subnets)
    }
}

impl Discovery<NetworkScanDiscovery> {
    /// Scan subnet concurrently and process hosts immediately as they're discovered
    async fn scan_and_process_hosts(
        &self,
        subnet: &Subnet,
        cancel: CancellationToken,
    ) -> Result<Vec<Host>> {
        tracing::info!(
            "Scanning subnet {} concurrently for hosts with open ports",
            subnet.base.cidr
        );

        let concurrent_scans = self.as_ref().config_store.get_concurrent_scans().await?;

        tracing::info!("Using up to {} concurrent scans", concurrent_scans);

        let session = self.as_ref().get_session().await?;

        let scanned_count = session.scanned_count.clone();
        let discovered_count: Arc<std::sync::atomic::AtomicUsize> =
            session.discovered_count.clone();

        // Report initial progress
        self.report_discovery_update(DiscoverySessionUpdate::scanning(0, 0))
            .await?;

        // Process all IPs concurrently, combining discovery and processing
        let results = stream::iter(self.determine_scan_order(&subnet.base.cidr))
            .map(async |ip| {
                let cancel = cancel.clone();
                let subnet = subnet.clone();
                let scanned_count = scanned_count.clone();

                match self.scan_host(ip, scanned_count, cancel).await {
                    Ok(None) => Ok(None),
                    Err(e) => Err(e),
                    Ok(Some((all_ports, endpoint_responses))) => {
                        let hostname = self.get_hostname_for_ip(ip).await?;

                        let mac = match subnet.base.subnet_type {
                            SubnetType::VpnTunnel => None, // ARP doesn't work through VPN tunnels
                            _ => self.as_ref().utils.get_mac_address_for_ip(ip).await?,
                        };

                        let interface = Interface::new(InterfaceBase {
                            name: None,
                            subnet_id: subnet.id,
                            ip_address: ip,
                            mac_address: mac,
                        });

                        if let Ok(Some((host, services))) = self
                            .process_host(
                                ServiceMatchBaselineParams {
                                    subnet: &subnet,
                                    interface: &interface,
                                    all_ports: &all_ports,
                                    endpoint_responses: &endpoint_responses,
                                    virtualization: &None,
                                },
                                hostname,
                            )
                            .await
                        {
                            discovered_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if let Ok((created_host, _)) = self.create_host(host, services).await {
                                return Ok::<Option<Host>, Error>(Some(created_host));
                            }
                        }
                        Ok(None)
                    }
                }
            })
            .buffer_unordered(concurrent_scans);

        // Consume the stream and report progress periodically
        tracing::info!(
            "Stream created for subnet {}, starting consumption",
            subnet.base.cidr
        );
        let mut stream_pin = Box::pin(results);
        let mut last_reported_scan_count: usize = 0;
        let mut last_reported_discovery_count: usize = 0;
        let mut successful_discoveries = Vec::new();

        while let Some(result) = stream_pin.next().await {
            if cancel.is_cancelled() {
                tracing::warn!("Discovery session was cancelled");
                return Err(Error::msg("Discovery session was cancelled"));
            }

            match result {
                Ok(Some(host)) => successful_discoveries.push(host),
                Ok(None) => {}
                Err(e) => {
                    // Check if this is a critical error (resource exhaustion)
                    if DiscoveryCriticalError::is_critical_error(e.to_string()) {
                        return Err(e); // Propagate the error up
                    } else {
                        // Non-critical errors just get logged
                        tracing::warn!("Error during scanning/processing: {}", e);
                    }
                }
            }

            (last_reported_scan_count, last_reported_discovery_count) = self
                .periodic_scan_update(20, last_reported_scan_count, last_reported_discovery_count)
                .await?;
        }

        tracing::info!("Completed scanning subnet {}", subnet.base.cidr);
        Ok(successful_discoveries)
    }

    pub async fn scan_host(
        &self,
        ip: IpAddr,
        scanned_count: Arc<std::sync::atomic::AtomicUsize>,
        cancel: CancellationToken,
    ) -> Result<Option<(Vec<PortBase>, Vec<EndpointResponse>)>> {
        // Check cancellation at the start
        if cancel.is_cancelled() {
            return Err(Error::msg("Discovery was cancelled"));
        }

        // Scan ports and endpoints
        let scan_result = tokio::spawn(Self::scan_ports_and_endpoints(ip, cancel.clone()))
            .await
            .map_err(|e| anyhow!("Scan task panicked: {}", e))?;

        // Check cancellation after network operation
        if cancel.is_cancelled() {
            scanned_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(Error::msg("Discovery was cancelled"));
        }

        match scan_result {
            Ok((open_ports, endpoint_responses)) => {
                if !open_ports.is_empty() || !endpoint_responses.is_empty() {
                    tracing::info!(
                        "Processing host {} with {} open ports and {} endpoint responses",
                        ip,
                        open_ports.len(),
                        endpoint_responses.len()
                    );

                    // Check cancellation before processing
                    if cancel.is_cancelled() {
                        scanned_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Err(Error::msg("Discovery was cancelled"));
                    }

                    Ok(Some((open_ports, endpoint_responses)))
                } else {
                    tracing::debug!("No open ports found on {}", ip);
                    scanned_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(None)
                }
            }
            Err(e) => {
                tracing::debug!("Error scanning host {}: {}", ip, e);

                if DiscoveryCriticalError::is_critical_error(e.to_string()) {
                    Err(e)
                } else {
                    scanned_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(None)
                }
            }
        }
    }

    async fn get_hostname_for_ip(&self, ip: IpAddr) -> Result<Option<String>, Error> {
        match timeout(SCAN_TIMEOUT, async {
            tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&ip)).await?
        })
        .await
        {
            Ok(Ok(hostname)) => Ok(Some(hostname)),
            _ => Ok(None),
        }
    }

    async fn scan_ports_and_endpoints(
        ip: IpAddr,
        cancel: CancellationToken,
    ) -> Result<(Vec<PortBase>, Vec<EndpointResponse>), Error> {
        if cancel.is_cancelled() {
            return Err(anyhow!("Operation cancelled"));
        }

        let mut open_ports = Vec::new();
        let mut endpoint_responses = Vec::new();

        // Scan TCP ports sequentially (not concurrently)
        let tcp_ports = Self::scan_tcp_ports(ip, cancel.clone()).await?;
        open_ports.extend(tcp_ports);

        if cancel.is_cancelled() {
            return Err(anyhow!("Operation cancelled"));
        }

        // Scan UDP ports sequentially
        let udp_ports = Self::scan_udp_ports(ip, cancel.clone()).await?;
        open_ports.extend(udp_ports);

        if cancel.is_cancelled() {
            return Err(anyhow!("Operation cancelled"));
        }

        // Scan endpoints sequentially
        let endpoints = Self::scan_endpoints(ip, cancel.clone()).await?;
        endpoint_responses.extend(endpoints);

        tracing::debug!(
            "Scan results for {}: found {} open ports, {} endpoint responses",
            ip,
            open_ports.len(),
            endpoint_responses.len()
        );

        Ok((open_ports, endpoint_responses))
    }

    async fn scan_tcp_ports(ip: IpAddr, cancel: CancellationToken) -> Result<Vec<PortBase>, Error> {
        let discovery_ports = Service::all_discovery_ports();
        let ports: Vec<u16> = discovery_ports
            .iter()
            .filter(|p| p.protocol() == TransportProtocol::Tcp)
            .map(|p| p.number())
            .collect();
        let mut open_ports = Vec::new();

        for port in ports {
            if cancel.is_cancelled() {
                break;
            }

            match timeout(SCAN_TIMEOUT, TcpStream::connect((ip, port))).await {
                Ok(Ok(_)) => {
                    open_ports.push(PortBase::new_tcp(port));
                    tracing::debug!("Found open TCP port {}:{}", ip, port);
                }
                Ok(Err(e)) => {
                    if DiscoveryCriticalError::is_critical_error(e.to_string()) {
                        return Err(e.into());
                    }
                }
                Err(_) => {
                    // Timeout - normal for closed/filtered ports
                }
            }
        }

        Ok(open_ports)
    }

    async fn scan_udp_ports(
        ip: IpAddr,
        cancel: CancellationToken,
    ) -> Result<Vec<PortBase>, anyhow::Error> {
        let discovery_ports = Service::all_discovery_ports();
        let ports: Vec<u16> = discovery_ports
            .iter()
            .filter(|p| p.protocol() == TransportProtocol::Udp)
            .map(|p| p.number())
            .collect();

        let mut open_ports = Vec::new();

        for port in ports {
            if cancel.is_cancelled() {
                break;
            }

            let result = match port {
                53 => Self::test_dns_service(ip).await,
                123 => Self::test_ntp_service(ip).await,
                161 => Self::test_snmp_service(ip).await,
                67 => Self::test_dhcp_service(ip).await,
                _ => Ok(None),
            };

            match result {
                Ok(Some(detected_port)) => {
                    open_ports.push(PortBase::new_udp(detected_port));
                    tracing::debug!("Found open UDP port {}:{}", ip, detected_port);
                }
                Ok(None) => {
                    // Port closed or no response
                }
                Err(e) => {
                    if DiscoveryCriticalError::is_critical_error(e.to_string()) {
                        return Err(e);
                    }
                }
            }
        }

        Ok(open_ports)
    }

    async fn scan_endpoints(
        ip: IpAddr,
        cancel: CancellationToken,
    ) -> Result<Vec<EndpointResponse>, Error> {
        let endpoints: Vec<Endpoint> = Service::all_discovery_endpoints()
            .iter()
            .map(|e| e.use_ip(ip))
            .collect();
        let mut responses = Vec::new();

        let client = reqwest::Client::builder()
            .timeout(SCAN_TIMEOUT) // Total request timeout
            .connect_timeout(Duration::from_millis(SCAN_TIMEOUT.as_millis() as u64 / 2)) // Half for connection
            .build()?;

        for endpoint in endpoints {
            if cancel.is_cancelled() {
                break;
            }

            let url = endpoint.to_string();

            match client.get(&url).send().await {
                Ok(response) => {
                    if let Ok(text) = response.text().await {
                        responses.push(EndpointResponse {
                            endpoint,
                            response: text,
                        });
                    }
                }
                Err(e) => {
                    if DiscoveryCriticalError::is_critical_error(e.to_string()) {
                        return Err(e.into());
                    }
                }
            }
        }

        Ok(responses)
    }

    /// Figure out what order to scan IPs in given allocation patterns
    fn determine_scan_order(&self, subnet: &IpCidr) -> impl Iterator<Item = IpAddr> {
        let mut ips: Vec<IpAddr> = subnet.iter().map(|ip| ip.address()).collect();

        // Sort by likelihood of being active hosts - highest probability first
        ips.sort_by_key(|ip| {
            let last_octet = match ip {
                IpAddr::V4(ipv4) => ipv4.octets()[3],
                IpAddr::V6(_) => return 9999, // IPv6 gets lowest priority for now
            };

            match last_octet {
                // Tier 1: Almost guaranteed to be active infrastructure
                1 => 1,   // Default gateway (.1)
                254 => 2, // Alternative gateway (.254)

                // Tier 2: Very common infrastructure and static assignments
                2 => 10,   // Secondary router/switch
                3 => 11,   // Tertiary infrastructure
                10 => 12,  // Common DHCP start
                100 => 13, // Common DHCP end
                253 => 14, // Alt gateway range
                252 => 15, // Alt gateway range

                // Tier 3: Common static device ranges
                4..=9 => 20 + last_octet as u16, // Infrastructure devices
                11..=20 => 30 + last_octet as u16, // Servers, printers
                21..=30 => 50 + last_octet as u16, // Network devices

                // Tier 4: Active DHCP ranges (most devices live here)
                31..=50 => 100 + last_octet as u16, // Early DHCP range
                51..=100 => 200 + last_octet as u16, // Mid DHCP range
                101..=150 => 400 + last_octet as u16, // Late DHCP range

                // Tier 5: Less common but still viable
                151..=200 => 600 + last_octet as u16, // Extended DHCP
                201..=251 => 800 + last_octet as u16, // High static range

                // Skip entirely - reserved addresses
                0 | 255 => 9998, // Network/broadcast addresses
            }
        });

        ips.into_iter()
    }

    // Use simpler DNS resolver that doesn't have API issues
    pub async fn test_dns_service(ip: IpAddr) -> Result<Option<u16>, Error> {
        // Use the simpler approach - create resolver with custom config directly
        let mut config = ResolverConfig::new();
        let name_server = NameServerConfig::new(SocketAddr::new(ip, 53), Protocol::Udp);
        config.add_name_server(name_server);

        let test_resolver = TokioAsyncResolver::tokio(config, ResolverOpts::default());

        match timeout(
            Duration::from_millis(2000),
            test_resolver.lookup_ip("google.com"),
        )
        .await
        {
            Ok(Ok(_)) => {
                tracing::debug!("DNS server responding at {}:53", ip);
                Ok(Some(53))
            }
            _ => {
                tracing::debug!("DNS server not responding at {}:53", ip);
                Ok(None)
            }
        }
    }

    pub async fn test_ntp_service(ip: IpAddr) -> Result<Option<u16>, Error> {
        let client = AsyncSntpClient::new();
        let server_addr = format!("{}:123", ip);

        match timeout(
            Duration::from_millis(2000),
            client.synchronize(&server_addr),
        )
        .await
        {
            Ok(Ok(result)) => {
                // Validate that we got a meaningful time response
                if let Ok(datetime) = result.datetime().unix_timestamp() {
                    if datetime > Duration::from_secs(0) {
                        // Sanity check for valid timestamp
                        tracing::debug!(
                            "NTP server responding at {}:123 with time {}",
                            ip,
                            datetime.as_millis()
                        );
                        Ok(Some(123))
                    } else {
                        tracing::debug!("Invalid NTP response from {}:123", ip);
                        Ok(None)
                    }
                } else {
                    tracing::debug!("Invalid NTP response from {}:123", ip);
                    Ok(None)
                }
            }
            Ok(Err(e)) => {
                tracing::debug!("NTP error from {}:123 - {}", ip, e);
                Ok(None)
            }
            Err(_) => {
                tracing::debug!("NTP timeout from {}:123", ip);
                Ok(None)
            }
        }
    }

    // Fixed: Add proper error handling and response validation
    pub async fn test_snmp_service(ip: IpAddr) -> Result<Option<u16>, Error> {
        let target = format!("{}:161", ip);
        let community = b"public";

        match AsyncSession::new_v2c(&target, community, 0).await {
            Ok(mut session) => {
                let sys_descr_oid = Oid::from(&[1, 3, 6, 1, 2, 1, 1, 1, 0]).unwrap();

                match timeout(Duration::from_millis(2000), session.get(&sys_descr_oid)).await {
                    Ok(Ok(mut response)) => {
                        if let Some(_varbind) = response.varbinds.next() {
                            tracing::debug!("SNMP server responding at {}:161", ip);
                            Ok(Some(161))
                        } else {
                            tracing::debug!("Empty SNMP response from {}:161", ip);
                            Ok(None)
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::debug!("SNMP error from {}:161 - {}", ip, e);
                        Ok(None)
                    }
                    Err(_) => {
                        tracing::debug!("SNMP timeout from {}:161", ip);
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                tracing::debug!("SNMP session creation failed for {}:161 - {}", ip, e);
                Ok(None)
            }
        }
    }

    /// Test if a host is running a DHCP server on port 67
    pub async fn test_dhcp_service(ip: IpAddr) -> Result<Option<u16>, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let target = SocketAddr::new(ip, 67);

        // Create a minimal DHCP DISCOVER message
        let mut rng = rand::rngs::StdRng::from_os_rng();
        let mac_addr: [u8; 6] = rng.random();
        let transaction_id = rng.random::<u32>();

        let mut msg = Message::default();
        msg.set_opcode(v4::Opcode::BootRequest)
            .set_htype(v4::HType::Eth)
            .set_xid(transaction_id)
            .set_flags(v4::Flags::default().set_broadcast())
            .set_chaddr(&mac_addr);

        msg.opts_mut()
            .insert(v4::DhcpOption::MessageType(MessageType::Discover));

        // Encode and send DHCP DISCOVER packet
        let mut buf = Vec::new();
        let mut encoder = Encoder::new(&mut buf);
        msg.encode(&mut encoder)?;
        socket.send_to(&buf, target).await?;

        // Wait for DHCP OFFER response
        let mut response_buf = [0u8; 1500];
        match timeout(
            Duration::from_millis(2000),
            socket.recv_from(&mut response_buf),
        )
        .await
        {
            Ok(Ok((len, _))) if len > 0 => {
                // Try to parse as DHCP message and validate response type
                match Message::decode(&mut dhcproto::Decoder::new(&response_buf[..len])) {
                    Ok(response_msg) => {
                        let is_valid_response = response_msg.opts().iter().any(|(_, opt)| {
                            matches!(
                                opt,
                                v4::DhcpOption::MessageType(MessageType::Offer)
                                    | v4::DhcpOption::MessageType(MessageType::Ack)
                            )
                        });

                        if is_valid_response {
                            tracing::debug!("DHCP server responding at {}:67", ip);
                            Ok(Some(67))
                        } else {
                            tracing::debug!("Invalid DHCP response from {}:67", ip);
                            Ok(None)
                        }
                    }
                    Err(_) => {
                        tracing::debug!("Failed to parse DHCP response from {}:67", ip);
                        Ok(None)
                    }
                }
            }
            _ => {
                tracing::debug!("DHCP timeout from {}:67", ip);
                Ok(None)
            }
        }
    }
}
