use crate::error::Error;
use crate::Action;

use serde::{Deserialize, Serialize};

use stream_api::{request::Request, FromMessage, IntoMessage};

use super::EmptyJson;

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct AccessPointsReq;

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct AccessPoints {
	pub device: String,
	pub list: Vec<AccessPoint>,
}

/// only returns ssids which have wpa-psk
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPoint {
	pub ssid: String,
	///
	pub strength: u8,
}

impl Request for AccessPointsReq {
	type Action = Action;
	type Response = AccessPoints;
	type Error = Error;

	const ACTION: Action = Action::NetworkAccessPoints;
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct ConnectionsReq;

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct Connections {
	pub list: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct Connection {
	pub id: String,
	pub uuid: String,
	pub kind: ConnectionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionKind {
	Wifi(ConnectionWifi),
	Gsm(ConnectionGsm),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionWifi {
	pub interface_name: String,
	/// can only be ssids which are wpa-psk
	pub ssid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionGsm {
	pub apn: String,
}

impl Request for ConnectionsReq {
	type Action = Action;
	type Response = Connections;
	type Error = Error;

	const ACTION: Action = Action::NetworkConnections;
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct AddConnectionReq {
	pub id: String,
	pub kind: AddConnectionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AddConnectionKind {
	Wifi(AddConnectionWifi),
	Gsm(AddConnectionGsm),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddConnectionWifi {
	pub interface_name: String,
	/// can only be ssids which are wpa-psk
	pub ssid: String,
	pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddConnectionGsm {
	pub apn: String,
}

impl Request for AddConnectionReq {
	type Action = Action;
	type Response = Connection;
	type Error = Error;

	const ACTION: Action = Action::NetworkAddConnection;
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoMessage, FromMessage)]
#[serde(rename_all = "camelCase")]
#[message(json)]
pub struct RemoveConnectionReq {
	pub uuid: String,
}

impl Request for RemoveConnectionReq {
	type Action = Action;
	type Response = EmptyJson;
	type Error = Error;

	const ACTION: Action = Action::NetworkRemoveConnection;
}
