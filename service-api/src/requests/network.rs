use crate::Action;
use crate::error::Error;

use serde::{Serialize, Deserialize};

use stream_api::request::Request;


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPointsReq;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPoints {
	pub device: String,
	pub list: Vec<AccessPoint>
}

/// only returns ssids which have wpa-psk
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPoint {
	pub ssid: String,
	/// 
	pub strength: u8
}

impl<B> Request<Action, B> for AccessPointsReq {
	type Response = AccessPoints;
	type Error = Error;

	const ACTION: Action = Action::NetworkAccessPoints;
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsReq;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connections {
	pub list: Vec<Connection>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
	pub id: String,
	pub uuid: String,
	pub kind: ConnectionKind
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionKind {
	Wifi(ConnectionWifi),
	// Gsm(ConnectionGsm)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionWifi {
	pub interface_name: String,
	/// can only be ssids which are wpa-psk
	pub ssid: String
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ConnectionGsm {
// }

impl<B> Request<Action, B> for ConnectionsReq {
	type Response = Connections;
	type Error = Error;

	const ACTION: Action = Action::NetworkConnections;
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddConnectionReq {
	pub id: String,
	pub kind: AddConnectionKind
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AddConnectionKind {
	Wifi(AddConnectionWifi),
	// Gsm(ConnectionGsm)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddConnectionWifi {
	pub interface_name: String,
	/// can only be ssids which are wpa-psk
	pub ssid: String,
	pub password: String
}

impl<B> Request<Action, B> for AddConnectionReq {
	type Response = Connection;
	type Error = Error;

	const ACTION: Action = Action::NetworkAddConnection;
}