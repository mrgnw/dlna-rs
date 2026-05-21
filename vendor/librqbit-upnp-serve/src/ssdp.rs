use std::{
	net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket},
	time::Duration,
};

use anyhow::{bail, Context};
use bstr::BStr;
use librqbit_dualstack_sockets::{MulticastOpts, MulticastUdpSocket};
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace, warn};

use crate::constants::{
	SSDP_ALL, UPNP_DEVICE_MEDIASERVER, UPNP_DEVICE_ROOT, UPNP_SERVICE_CONNECTION_MANAGER,
	UPNP_SERVICE_CONTENT_DIRECTORY,
};

const SSDP_PORT: u16 = 1900;
const SSDP_MCAST_IPV4: SocketAddrV4 =
	SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), SSDP_PORT);
#[allow(unused)]
const SSDP_MCAST_IPV6_LINK_LOCAL: SocketAddrV6 = SocketAddrV6::new(
	Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xc),
	SSDP_PORT,
	0,
	0,
);
const SSDP_MCAST_IPV6_SITE_LOCAL: SocketAddrV6 = SocketAddrV6::new(
	Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 0xc),
	SSDP_PORT,
	0,
	0,
);

const NTS_ALIVE: &str = "ssdp:alive";
const NTS_BYEBYE: &str = "ssdp:byebye";

#[derive(Debug)]
pub enum SsdpMessage<'a, 'h> {
	MSearch(SsdpMSearchRequest<'a>),
	#[allow(dead_code)]
	OtherRequest(httparse::Request<'h, 'a>),
	#[allow(dead_code)]
	Response(httparse::Response<'h, 'a>),
}

#[derive(Debug)]
pub struct SsdpMSearchRequest<'a> {
	#[allow(dead_code)]
	pub host: &'a BStr,
	pub man: &'a BStr,
	pub st: &'a BStr,
}

impl SsdpMSearchRequest<'_> {
	fn matches_media_server(&self) -> bool {
		if self.man != "\"ssdp:discover\"" {
			return false;
		}
		if self.st == SSDP_ALL
			|| self.st == UPNP_DEVICE_ROOT
			|| self.st == UPNP_DEVICE_MEDIASERVER
			|| self.st == UPNP_SERVICE_CONTENT_DIRECTORY
			|| self.st == UPNP_SERVICE_CONNECTION_MANAGER
		{
			return true;
		}
		false
	}
}

pub fn try_parse_ssdp<'a, 'h>(
	buf: &'a [u8],
	headers: &'h mut [httparse::Header<'a>],
) -> anyhow::Result<SsdpMessage<'a, 'h>> {
	if buf.starts_with(b"HTTP/") {
		let mut resp = httparse::Response::new(headers);
		resp.parse(buf).context("error parsing response")?;
		return Ok(SsdpMessage::Response(resp));
	}

	let mut req = httparse::Request::new(headers);
	req.parse(buf).context("error parsing request")?;

	match req.method {
		Some("M-SEARCH") => {
			let mut host = None;
			let mut man = None;
			let mut st = None;

			for header in req.headers.iter() {
				match header.name {
					"HOST" | "Host" | "host" => host = Some(header.value),
					"MAN" | "Man" | "man" => man = Some(header.value),
					"ST" | "St" | "st" => st = Some(header.value),
					other => trace!(header=?BStr::new(other), "ignoring SSDP header"),
				}
			}

			match (host, man, st) {
				(Some(host), Some(man), Some(st)) => Ok(SsdpMessage::MSearch(SsdpMSearchRequest {
					host: BStr::new(host),
					man: BStr::new(man),
					st: BStr::new(st),
				})),
				_ => bail!("not all of host, man and st are set"),
			}
		}
		_ => Ok(SsdpMessage::OtherRequest(req)),
	}
}

pub struct SsdpRunnerOptions {
	pub usn: String,
	pub description_http_location: url::Url,
	pub server_string: String,
	pub notify_interval: Duration,
	pub shutdown: CancellationToken,
}

pub struct SsdpRunner {
	opts: SsdpRunnerOptions,
	socket: MulticastUdpSocket,
}

fn advertised_targets() -> Vec<&'static str> {
	vec![
		UPNP_DEVICE_ROOT,
		UPNP_DEVICE_MEDIASERVER,
		UPNP_SERVICE_CONTENT_DIRECTORY,
		UPNP_SERVICE_CONNECTION_MANAGER,
	]
}

impl SsdpRunner {
	pub async fn new(opts: SsdpRunnerOptions) -> anyhow::Result<Self> {
		let socket = MulticastUdpSocket::new(
			(Ipv4Addr::UNSPECIFIED, SSDP_PORT).into(),
			SSDP_MCAST_IPV4,
			SSDP_MCAST_IPV6_SITE_LOCAL,
			None,
			// Some(SSDP_MCAST_IPV6_LINK_LOCAL),
			None,
		)
		.await
		.context("error creating SSDP socket")?;

		Ok(Self { opts, socket })
	}

	fn generate_notify_message(
		&self,
		device_kind: &str,
		nts: &str,
		opts: &MulticastOpts,
	) -> Option<String> {
		if !is_discoverable_local_iface_ip(opts.iface_ip(), self.socket.nics()) {
			trace!(?opts, "skipping SSDP notify on non-discoverable interface");
			return None;
		}
		let usn: &str = &self.opts.usn;
		let server: &str = &self.opts.server_string;
		let host = addr_no_scope(&opts.mcast_addr());
		let mut location = self.opts.description_http_location.clone();
		let _ = location.set_ip_host(opts.iface_ip());
		Some(format!(
			"NOTIFY * HTTP/1.1\r
Host: {host}\r
Cache-Control: max-age=75\r
Location: {location}\r
NT: {device_kind}\r
NTS: {nts}\r
Server: {server}\r
USN: {usn}::{device_kind}\r
\r
"
		))
	}

	fn generate_ssdp_discover_response(
		&self,
		st: &str,
		addr: SocketAddr,
	) -> anyhow::Result<Option<String>> {
		if matches!(addr.ip(), IpAddr::V6(a) if a.is_unicast_link_local()) {
			// VLC doesn't work with link-local URLs no matter what I tried. Furthermore, it probably
			// wants an interface name in its scope id, which we of course don't know as its local to
			// the client.
			debug!(?addr, "refusing to reply to a link-local address");
			return Ok(None);
		}
		let local_ip = if addr.ip().is_loopback() {
			private_ipv4_addr(self.socket.nics())
				.or_else(|| local_ip_for_unicast_reply(addr))
				.map(Ok)
				.unwrap_or_else(|| {
					::librqbit_upnp::get_local_ip_relative_to(addr, self.socket.nics())
				})?
		} else {
			local_ip_for_unicast_reply(addr)
				.map(Ok)
				.unwrap_or_else(|| {
					::librqbit_upnp::get_local_ip_relative_to(addr, self.socket.nics())
				})?
		};
		let location = {
			let mut loc = self.opts.description_http_location.clone();
			let _ = loc.set_ip_host(local_ip);
			loc
		};
		let usn = &self.opts.usn;
		let server = &self.opts.server_string;
		Ok(Some(format!(
			"HTTP/1.1 200 OK\r
Cache-Control: max-age=75\r
Ext: \r
Location: {location}\r
Server: {server}\r
St: {st}\r
Usn: {usn}::{st}\r
Content-Length: 0\r\n\r\n"
		)))
	}

	async fn try_send_notifies(&self, nts: &str) {
		for target in advertised_targets() {
			self.socket
				.try_send_mcast_everywhere(&|opts| {
					self.generate_notify_message(target, nts, opts)
				})
				.await;
		}
	}

	async fn task_send_alive_notifies_periodically(&self) {
		let mut interval = tokio::time::interval(self.opts.notify_interval);
		loop {
			interval.tick().await;
			self.try_send_notifies(NTS_ALIVE).await;
		}
	}

	async fn process_incoming_message(&self, msg: &[u8], addr: SocketAddr) -> anyhow::Result<()> {
		if is_excluded_local_iface_ip(addr.ip(), self.socket.nics()) {
			trace!(
				?addr,
				"ignoring SSDP message from non-discoverable local interface"
			);
			return Ok(());
		}
		let mut headers = [httparse::EMPTY_HEADER; 16];
		trace!(content = ?BStr::new(msg), ?addr, "received message");
		let parsed = try_parse_ssdp(msg, &mut headers);
		let msg = match parsed {
			Ok(SsdpMessage::MSearch(msg)) => msg,
			Ok(m) => {
				trace!("ignoring {m:?}");
				return Ok(());
			}
			Err(e) => {
				debug!(error=?e, "error parsing SSDP message");
				return Ok(());
			}
		};
		if !msg.matches_media_server() {
			trace!("not a media server request, ignoring");
			return Ok(());
		}

		if let Ok(st) = std::str::from_utf8(msg.st) {
			let targets = if st == SSDP_ALL {
				advertised_targets()
			} else {
				vec![st]
			};
			for target in targets {
				let response = self.generate_ssdp_discover_response(target, addr)?;
				if let Some(response) = response {
					trace!(content = response, ?addr, "sending SSDP discover response");
					self.socket
						.send_to(response.as_bytes(), addr)
						.await
						.context("error sending")?;
				}
			}
		}

		Ok(())
	}

	async fn task_respond_on_msearches(&self) {
		let mut buf = vec![0u8; 16184];

		loop {
			let (sz, addr) = match self.socket.recv_from(&mut buf).await {
				Ok((sz, addr)) => (sz, addr),
				Err(e) => {
					warn!(error=?e, "error receiving");
					return;
				}
			};
			let msg = &buf[..sz];
			if let Err(e) = self.process_incoming_message(msg, addr).await {
				warn!(error=?e, ?addr, "error processing incoming SSDP message")
			}
		}
	}

	async fn try_send_example_msearch(&self) {
		self.socket
			.try_send_mcast_everywhere(&|opts| {
				if !is_discoverable_local_iface_ip(opts.iface_ip(), self.socket.nics()) {
					trace!(
						?opts,
						"skipping SSDP m-search on non-discoverable interface"
					);
					return None;
				}
				let dest = addr_no_scope(&opts.mcast_addr());
				Some(format!(
					"M-SEARCH * HTTP/1.1\r
HOST: {dest}\r
ST: urn:schemas-upnp-org:device:MediaServer:1\r
MAN: \"ssdp:discover\"\r
MX: 2\r\n\r\n"
				))
			})
			.await
	}

	pub async fn run_forever(&self) -> anyhow::Result<()> {
		// This isn't necessary, but would show that it works.
		let t0 = self.try_send_example_msearch();
		let t1 = self.task_respond_on_msearches();
		let t2 = self.task_send_alive_notifies_periodically();

		let wait = async move {
			tokio::join!(t0, t1, t2);
			Ok(())
		};

		tokio::select! {
			r = wait => r,
			_ = self.opts.shutdown.cancelled() => {
				self.try_send_notifies(NTS_BYEBYE).await;
				Ok(())
			}
		}
	}
}

fn addr_no_scope(addr: &SocketAddr) -> SocketAddr {
	match addr {
		SocketAddr::V4(a) => SocketAddr::V4(*a),
		SocketAddr::V6(a) => {
			let mut a = *a;
			a.set_scope_id(0);
			SocketAddr::V6(a)
		}
	}
}

fn local_ip_for_unicast_reply(addr: SocketAddr) -> Option<IpAddr> {
	let bind_addr = match addr {
		SocketAddr::V4(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
		SocketAddr::V6(_) => SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0),
	};
	let socket = UdpSocket::bind(bind_addr).ok()?;
	socket.connect(addr).ok()?;
	socket.local_addr().ok().map(|addr| addr.ip())
}

fn private_ipv4_addr(nics: &[network_interface::NetworkInterface]) -> Option<IpAddr> {
	nics.iter()
		.filter(|nic| is_discoverable_nic(&nic.name))
		.flat_map(|nic| nic.addr.iter())
		.filter_map(|addr| match addr.ip() {
			IpAddr::V4(ip) if ip.is_private() && !ip.is_loopback() => Some(IpAddr::V4(ip)),
			_ => None,
		})
		.next()
}

fn is_discoverable_local_iface_ip(
	ip: IpAddr,
	nics: &[network_interface::NetworkInterface],
) -> bool {
	if !is_advertisable_ip(ip) {
		return false;
	}
	nics.iter()
		.filter(|nic| is_discoverable_nic(&nic.name))
		.flat_map(|nic| nic.addr.iter())
		.any(|addr| addr.ip() == ip)
}

fn is_excluded_local_iface_ip(ip: IpAddr, nics: &[network_interface::NetworkInterface]) -> bool {
	nics.iter()
		.filter(|nic| !is_discoverable_nic(&nic.name))
		.flat_map(|nic| nic.addr.iter())
		.any(|addr| addr.ip() == ip)
}

/// Whether an interface IP is suitable to advertise as a UPnP `Location`.
///
/// Rejects loopback, link-local, unspecified, multicast/broadcast, and the
/// CGNAT range 100.64.0.0/10 used by Tailscale and carrier NAT — none of which
/// a LAN renderer (e.g. an Apple TV) can route back to.
pub fn is_advertisable_ip(ip: IpAddr) -> bool {
	match ip {
		IpAddr::V4(ip) => {
			if ip.is_loopback()
				|| ip.is_link_local()
				|| ip.is_unspecified()
				|| ip.is_broadcast()
				|| ip.is_multicast()
			{
				return false;
			}
			let octets = ip.octets();
			let is_cgnat = octets[0] == 100 && (octets[1] & 0xc0) == 0x40;
			!is_cgnat
		}
		IpAddr::V6(ip) => {
			let is_link_local = (ip.segments()[0] & 0xffc0) == 0xfe80;
			!(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || is_link_local)
		}
	}
}

fn is_discoverable_nic(name: &str) -> bool {
	!(name.starts_with("bridge")
		|| name.starts_with("vmenet")
		|| name.starts_with("utun")
		|| name.starts_with("awdl")
		|| name.starts_with("llw"))
}
