use std::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;

use dbus::{Error, Path};
use dbus::blocking::{Connection as DbusConnection, Proxy};
use dbus::arg::{RefArg, Variant};

use nmdbus::NetworkManager as DbusNetworkManager;
use nmdbus::device::Device as DeviceTrait;
use nmdbus::device_wireless::DeviceWireless;
use nmdbus::accesspoint::AccessPoint as AccessPointTrait;
use nmdbus::settings::Settings as SettingsTrait;
use nmdbus::settings_connection::SettingsConnection as SettingsConnectionTrait;

use serde::{Serialize, Deserialize};

const DBUS_NAME: &str = "org.freedesktop.NetworkManager";
const DBUS_PATH: &str = "/org/freedesktop/NetworkManager";
const DBUS_PATH_SETTINGS: &str = "/org/freedesktop/NetworkManager/Settings";
const TIMEOUT: Duration = Duration::from_secs(2);

/*
 let response = self.dbus
    .call(NM_SETTINGS_PATH, NM_SETTINGS_INTERFACE, "ListConnections")?;

let array: Array<Path, _> = self.dbus.extract(&response)?;

Ok(array.map(|e| e.to_string()).collect())
*/


#[derive(Clone)]
struct Dbus {
	conn: Arc<DbusConnection>
}

impl Dbus {
	fn connect() -> Result<Self, Error> {
		DbusConnection::new_system()
			.map(Arc::new)
			.map(|conn| Self { conn })
	}

	fn proxy<'a, 'b>(
		&'b self,
		path: impl Into<Path<'a>>
	) -> Proxy<'a, &'b DbusConnection> {
		self.conn.with_proxy(DBUS_NAME, path, TIMEOUT)
	}
}


#[derive(Clone)]
pub struct NetworkManager {
	dbus: Dbus
}

impl NetworkManager {
	pub fn connect() -> Result<Self, Error> {
		Dbus::connect()
			.map(|dbus| Self { dbus })
	}

	pub fn devices(&self) -> Result<Vec<Device>, Error> {
		let paths = self.dbus.proxy(DBUS_PATH).get_devices()?;
		let devices = paths.into_iter()
			.map(|path| {
				Device {
					dbus: self.dbus.clone(),
					path
				}
			})
			.collect();

		Ok(devices)
	}

	pub fn connections(&self) -> Result<Vec<Connection>, Error> {
		let paths = self.dbus.proxy(DBUS_PATH_SETTINGS).list_connections()?;
		let connections = paths.into_iter()
			.map(|path| {
				Connection {
					dbus: self.dbus.clone(),
					path
				}
			})
			.collect();

		Ok(connections)
	}

	pub fn add_connection(
		&self,
		connection: HashMap<&str, PropMap>
	) -> Result<Connection, Error> {
		let connection = connection.into_iter()
			.map(|(k, v)| (k, v.inner))
			.collect();

		self.dbus.proxy(DBUS_PATH_SETTINGS)
			.add_connection(connection)
			.map(|path| {
				Connection {
					dbus: self.dbus.clone(),
					path
				}
			})
	}

	pub fn remove_connection(&self, uuid: &str) -> Result<(), Error> {
		let path = self.dbus.proxy(DBUS_PATH_SETTINGS)
			.get_connection_by_uuid(uuid)?;

		let con = Connection {
			dbus: self.dbus.clone(),
			path
		};

		con.delete()
	}
}

pub struct Device {
	dbus: Dbus,
	path: Path<'static>
}

impl Device {
	pub fn interface(&self) -> Result<String, Error> {
		self.dbus.proxy(&self.path).interface()
	}

	// /// The path of the device as exposed by the udev property ID_PATH.  
	// /// Note that non-UTF-8 characters are backslash escaped.
	// /// Use g_strcompress() to obtain the true (non-UTF-8) string. 
	// pub fn path(&self) -> Result<String, Error> {
	// 	self.dbus.proxy(&self.path).path()
	// }

	/// The general type of the network device; ie Ethernet, Wi-Fi, etc.
	pub fn kind(&self) -> Result<DeviceKind, Error> {
		self.dbus.proxy(&self.path).device_type()
			.map(Into::into)
	}

	/// make sure you call a device with Wifi
	pub fn access_points(&self) -> Result<Vec<AccessPoint>, Error> {
		self.dbus.proxy(&self.path).request_scan(HashMap::new())?;

		self.dbus.proxy(&self.path).get_all_access_points()
			.map(|paths| {
				paths.into_iter()
					.map(|path| {
						AccessPoint {
							dbus: self.dbus.clone(),
							path
						}
					})
					.collect()
			})
	}
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
	/// unknown device
	Unknown = 0,
	/// generic support for unrecognized device types
	Generic = 14,
	/// a wired ethernet device
	Ethernet = 1,
	/// an 802.11 Wi-Fi device
	Wifi = 2,
	/// not used
	Unused1 = 3,
	/// not used
	Unused2 = 4,
	/// a Bluetooth device supporting PAN or DUN access protocols
	Bt = 5,
	/// an OLPC XO mesh networking device
	OlpcMesh = 6,
	/// an 802.16e Mobile WiMAX broadband device
	Wimax = 7,
	/// a modem supporting analog telephone, CDMA/EVDO, GSM/UMTS,
	/// or LTE network access protocols
	Modem = 8,
	/// an IP-over-InfiniBand device
	Infiniband = 9,
	/// a bond master interface
	Bond = 10,
	/// an 802.1Q VLAN interface
	Vlan = 11,
	/// ADSL modem
	Adsl = 12,
	/// a bridge master interface
	Bridge = 13,
	/// a team master interface
	Team = 15,
	/// a TUN or TAP interface
	Tun = 16,
	/// a IP tunnel interface
	IpTunnel = 17,
	/// a MACVLAN interface
	Macvlan = 18,
	/// a VXLAN interface
	Vxlan = 19,
	/// a VETH interface
	Veth = 20,
	/// a MACsec interface
	Macsec = 21,
	/// a dummy interface
	Dummy = 22,
	/// a PPP interface
	Ppp = 23,
	/// a Open vSwitch interface
	OvsInterface = 24,
	/// a Open vSwitch port
	OvsPort = 25,
	/// a Open vSwitch bridge
	OvsBridge = 26,
	/// a IEEE 802.15.4 (WPAN) MAC Layer Device
	Wpan = 27,
	/// 6LoWPAN interface
	SixLowPan = 28,
	/// a WireGuard interface
	Wireguard = 29,
	/// an 802.11 Wi-Fi P2P device. Since: 1.16.
	WifiP2p = 30,
	/// A VRF (Virtual Routing and Forwarding) interface. Since: 1.24.
	Vrf = 31
}

impl From<u32> for DeviceKind {
	fn from(num: u32) -> Self {
		if num > 31 {
			Self::Unknown
		} else {
			unsafe {
				*(&num as *const u32 as *const Self)
			}
		}
	}
}

pub struct AccessPoint {
	dbus: Dbus,
	path: Path<'static>
}

impl AccessPoint {
	pub fn wpa_flags(&self) -> Result<ApSecurityFlags, Error> {
		self.dbus.proxy(&self.path).wpa_flags()
			.map(Into::into)
	}

	pub fn rsn_flags(&self) -> Result<ApSecurityFlags, Error> {
		self.dbus.proxy(&self.path).rsn_flags()
			.map(Into::into)
	}

	pub fn ssid(&self) -> Result<String, Error> {
		self.dbus.proxy(&self.path).ssid()
			.and_then(|b| {
				String::from_utf8(b)
					.map_err(|_| Error::new_failed("ssid not valid utf8"))
			})
	}

	#[allow(dead_code)]
	pub fn mode(&self) -> Result<ApMode, Error> {
		AccessPointTrait::mode(&self.dbus.proxy(&self.path))
			.map(Into::into)
	}

	pub fn strength(&self) -> Result<u8, Error> {
		self.dbus.proxy(&self.path).strength()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApSecurityFlags(u32);

impl ApSecurityFlags {
	pub fn matches(&self, flag: ApSecurityFlag) -> bool {
		self.0 & flag as u32 > 0
	}
}

impl From<u32> for ApSecurityFlags {
	fn from(n: u32) -> Self {
		Self(n)
	}
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApSecurityFlag {
	None = 0,
	KeyMgmtPsk = 0x00000100,
	KeyMgmt802_1x = 0x00000200
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApMode {
	Unknown = 0,
	Adhoc = 1,
	Infra = 2,
	Ap = 3
}

impl From<u32> for ApMode {
	fn from(num: u32) -> Self {
		if num > 3 {
			Self::Unknown
		} else {
			unsafe {
				*(&num as *const u32 as *const Self)
			}
		}
	}
}


pub struct Connection {
	dbus: Dbus,
	path: Path<'static>
}

impl Connection {
	pub fn get_settings(&self) -> Result<HashMap<String, PropMap>, Error> {
		self.dbus.proxy(&self.path).get_settings()
			.map(|map| {
				map.into_iter()
					.map(|(k, v)| (k, PropMap { inner: v }))
					.collect()
			})
	}

	pub fn delete(&self) -> Result<(), Error> {
		SettingsConnectionTrait::delete(&self.dbus.proxy(&self.path))
	}
}

#[derive(Debug)]
pub struct PropMap {
	inner: HashMap<String, Variant<Box<dyn RefArg + 'static>>>
}

impl PropMap {
	pub fn new() -> Self {
		Self {
			inner: HashMap::new()
		}
	}

	pub fn get_str(&self, s: &str) -> Option<&str> {
		self.inner.get(s)?
			.as_str()
	}

	pub fn get_string_from_bytes(&self, s: &str) -> Option<String> {
		let bytes: Vec<u8> = self.inner.get(s)?.0
			.as_iter()?
			// this should be done better
			.map(|a| a.as_u64().and_then(|u| u.try_into().ok()))
			.collect::<Option<_>>()?;

		String::from_utf8(bytes).ok()
	}

	pub fn insert_string(&mut self, k: impl Into<String>, v: impl Into<String>) {
		self.inner.insert(k.into(), Variant(Box::new(v.into())));
	}

	pub fn insert_string_as_bytes(
		&mut self,
		k: impl Into<String>,
		v: impl Into<String>
	) {
		let v = v.into().into_bytes();
		self.inner.insert(k.into(), Variant(Box::new(v)));
	}
}