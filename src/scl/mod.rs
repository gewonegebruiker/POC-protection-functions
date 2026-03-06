//! SCL (Substation Configuration Language) / SCD file support.
//!
//! Provides type definitions and a parser skeleton for IEC 61850 SCD files.

pub mod types;
pub mod parser;

pub use types::{
    ScdFile, SclHeader, Communication, SubNetwork, ConnectedAP,
    GseAddress, SmvAddress, IedDefinition, AccessPoint, Server,
    LogicalDevice, LogicalNode, DataObject, DataAttribute,
    ProtectionFunctionConfig, IedConfig, SvSubscription, GoosePublication,
    SclSystemConfig,
};
pub use parser::SclParser;
