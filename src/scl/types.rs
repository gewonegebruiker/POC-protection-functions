//! SCL/SCD type definitions for IEC 61850 substation configuration.

use serde::{Deserialize, Serialize};
use crate::config::{PtocConfig, PiocConfig, GooseConfig, SvConfig, CtConfig, AdcConfig};

// ---------------------------------------------------------------------------
// Top-level SCD file structure
// ---------------------------------------------------------------------------

/// Top-level representation of a parsed SCD (Substation Configuration Description) file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScdFile {
    pub header: SclHeader,
    pub communication: Communication,
    pub ieds: Vec<IedDefinition>,
}

/// SCL file header metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SclHeader {
    pub id: String,
    pub version: String,
    pub revision: String,
}

// ---------------------------------------------------------------------------
// Communication section
// ---------------------------------------------------------------------------

/// Communication section — sub-networks and connected access points.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Communication {
    pub sub_networks: Vec<SubNetwork>,
}

/// A sub-network (e.g., process bus or station bus).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubNetwork {
    pub name: String,
    pub r#type: String,
    pub connected_aps: Vec<ConnectedAP>,
}

/// A connected access point referencing an IED and its access point name.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectedAP {
    pub ied_name: String,
    pub ap_name: String,
    pub gse_addresses: Vec<GseAddress>,
    pub smv_addresses: Vec<SmvAddress>,
}

/// GSE (GOOSE) address entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GseAddress {
    pub cb_name: String,
    pub mac: String,
    pub appid: u16,
    pub vlan_id: u16,
    pub vlan_priority: u8,
    pub min_time_ms: u32,
    pub max_time_ms: u32,
}

/// SMV (Sampled Values) address entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SmvAddress {
    pub cb_name: String,
    pub mac: String,
    pub appid: u16,
    pub vlan_id: u16,
    pub vlan_priority: u8,
}

// ---------------------------------------------------------------------------
// IED section
// ---------------------------------------------------------------------------

/// Full IED definition from the SCL file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IedDefinition {
    pub name: String,
    pub r#type: String,
    pub access_points: Vec<AccessPoint>,
}

/// Access point within an IED.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessPoint {
    pub name: String,
    pub server: Server,
}

/// Server containing one or more logical devices.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Server {
    pub logical_devices: Vec<LogicalDevice>,
}

/// Logical device (LD) within the server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogicalDevice {
    pub inst: String,
    pub logical_nodes: Vec<LogicalNode>,
}

/// Logical node (LN) — e.g., PTOC1, PIOC1.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogicalNode {
    pub ln_class: String,
    pub inst: String,
    pub data_objects: Vec<DataObject>,
}

/// Data object within a logical node.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataObject {
    pub name: String,
    pub data_attributes: Vec<DataAttribute>,
}

/// Data attribute within a data object.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataAttribute {
    pub name: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Application-level config derived from SCL
// ---------------------------------------------------------------------------

/// Protection function configuration extracted from SCL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtectionFunctionConfig {
    Ptoc(PtocConfig),
    Pioc(PiocConfig),
}

/// SV subscription entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SvSubscription {
    pub stream_id: String,
    pub interface: String,
    pub multicast_mac: String,
    pub samples_per_cycle: usize,
}

/// GOOSE publication entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoosePublication {
    pub cb_ref: String,
    pub dst_mac: String,
    pub appid: u16,
    pub interface: String,
}

/// IED-level configuration derived from the SCL/SCD file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IedConfig {
    pub ied_name: String,
    pub protection_functions: Vec<ProtectionFunctionConfig>,
    pub sv_subscriptions: Vec<SvSubscription>,
    pub goose_publications: Vec<GoosePublication>,
    pub ct: CtConfig,
    pub adc: AdcConfig,
}

/// System-wide configuration used at runtime — mirrors `crate::config::SystemConfig`.
///
/// This is the SCL-module view; use `SclParser::to_system_config` to convert to
/// the canonical `crate::config::SystemConfig` used by the rest of the codebase.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SclSystemConfig {
    pub ptoc: PtocConfig,
    pub pioc: PiocConfig,
    pub ct: CtConfig,
    pub adc: AdcConfig,
    pub goose: GooseConfig,
    pub sv: SvConfig,
}
