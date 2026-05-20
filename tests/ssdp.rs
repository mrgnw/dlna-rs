use librqbit_upnp_serve::is_advertisable_ip;
use std::net::{IpAddr, Ipv6Addr};

fn v4(s: &str) -> IpAddr {
	IpAddr::V4(s.parse().unwrap())
}

#[test]
fn accepts_private_lan_ipv4() {
	assert!(is_advertisable_ip(v4("192.168.8.154")));
	assert!(is_advertisable_ip(v4("10.0.0.5")));
	assert!(is_advertisable_ip(v4("172.16.3.4")));
}

#[test]
fn rejects_tailscale_cgnat() {
	assert!(!is_advertisable_ip(v4("100.64.212.2")));
	assert!(!is_advertisable_ip(v4("100.127.255.254")));
}

#[test]
fn rejects_loopback_linklocal_unspecified() {
	assert!(!is_advertisable_ip(v4("127.0.0.1")));
	assert!(!is_advertisable_ip(v4("169.254.1.1")));
	assert!(!is_advertisable_ip(v4("0.0.0.0")));
	assert!(!is_advertisable_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
	assert!(!is_advertisable_ip(IpAddr::V6("fe80::1".parse().unwrap())));
}
