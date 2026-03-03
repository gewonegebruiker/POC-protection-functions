//! SCL/SCD parser skeleton.
//!
//! The actual XML parsing is deferred (`todo!()`) until an SCD file is
//! available.  The `to_system_config` bridge function is fully implemented
//! to map the SCL-derived `IedConfig` to the runtime `crate::config::SystemConfig`.

use super::types::{IedConfig, ProtectionFunctionConfig};
use crate::config::{PtocConfig, PiocConfig, SystemConfig};

/// Result type used throughout the parser.
pub type ParseResult<T> = Result<T, Box<dyn std::error::Error>>;

/// SCL/SCD parser.
pub struct SclParser;

impl SclParser {
    /// Parse an SCD file and extract the configuration for a named IED.
    ///
    /// # Arguments
    /// * `scd_path` – Path to the `.scd` file.
    /// * `ied_name` – Name of the IED to extract (e.g., `"BAY1_IED"`).
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or the IED is not found.
    ///
    /// > **Note**: XML parsing is not yet implemented; call sites should
    /// > supply a pre-built `IedConfig` or wait for a future implementation.
    pub fn parse_for_ied(_scd_path: &str, _ied_name: &str) -> ParseResult<IedConfig> {
        todo!("XML parsing will be implemented once an SCD file is available")
    }

    /// Convert an `IedConfig` (from SCL) to the runtime `SystemConfig`.
    ///
    /// The first PTOC/PIOC protection function found is used; defaults are
    /// applied for any missing parameters.
    pub fn to_system_config(ied_config: &IedConfig) -> SystemConfig {
        let mut ptoc = PtocConfig::default();
        let mut pioc = PiocConfig::default();

        for pf in &ied_config.protection_functions {
            match pf {
                ProtectionFunctionConfig::Ptoc(p) => ptoc = p.clone(),
                ProtectionFunctionConfig::Pioc(p) => pioc = p.clone(),
            }
        }

        // Map the first SV subscription to SvConfig
        let sv = if let Some(sub) = ied_config.sv_subscriptions.first() {
            crate::config::SvConfig {
                samples_per_cycle: if sub.samples_per_cycle > 0 {
                    sub.samples_per_cycle
                } else {
                    80
                },
                interface: sub.interface.clone(),
                multicast_mac: sub.multicast_mac.clone(),
            }
        } else {
            crate::config::SvConfig::default()
        };

        // Map the first GOOSE publication to GooseConfig
        let goose = if let Some(pub_) = ied_config.goose_publications.first() {
            crate::config::GooseConfig {
                dst_mac: pub_.dst_mac.clone(),
                appid: pub_.appid,
                interface: pub_.interface.clone(),
                ..crate::config::GooseConfig::default()
            }
        } else {
            crate::config::GooseConfig::default()
        };

        SystemConfig {
            ptoc,
            pioc,
            ct: ied_config.ct.clone(),
            adc: ied_config.adc.clone(),
            goose,
            sv,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scl::types::{GoosePublication, SvSubscription};

    #[test]
    fn test_to_system_config_defaults() {
        let ied_config = IedConfig::default();
        let sys = SclParser::to_system_config(&ied_config);
        assert_eq!(sys.ptoc.iset, PtocConfig::default().iset);
        assert_eq!(sys.pioc.iset, PiocConfig::default().iset);
    }

    #[test]
    fn test_to_system_config_with_ptoc() {
        let mut ied_config = IedConfig::default();
        ied_config.protection_functions.push(ProtectionFunctionConfig::Ptoc(PtocConfig {
            iset: 200.0,
            tset: 50,
            enabled: true,
        }));
        let sys = SclParser::to_system_config(&ied_config);
        assert_eq!(sys.ptoc.iset, 200.0);
        assert_eq!(sys.ptoc.tset, 50);
    }

    #[test]
    fn test_to_system_config_with_pioc() {
        let mut ied_config = IedConfig::default();
        ied_config.protection_functions.push(ProtectionFunctionConfig::Pioc(PiocConfig {
            iset: 800.0,
            enabled: true,
        }));
        let sys = SclParser::to_system_config(&ied_config);
        assert_eq!(sys.pioc.iset, 800.0);
    }

    #[test]
    fn test_to_system_config_sv_mapping() {
        let mut ied_config = IedConfig::default();
        ied_config.sv_subscriptions.push(SvSubscription {
            stream_id: "SV1".to_string(),
            interface: "eth1".to_string(),
            multicast_mac: "01:0C:CD:04:00:01".to_string(),
            samples_per_cycle: 80,
        });
        let sys = SclParser::to_system_config(&ied_config);
        assert_eq!(sys.sv.interface, "eth1");
        assert_eq!(sys.sv.samples_per_cycle, 80);
    }

    #[test]
    fn test_to_system_config_goose_mapping() {
        let mut ied_config = IedConfig::default();
        ied_config.goose_publications.push(GoosePublication {
            cb_ref: "IED1LD0/LLN0$GO$GCB1".to_string(),
            dst_mac: "01:0C:CD:01:00:10".to_string(),
            appid: 0x0010,
            interface: "eth0".to_string(),
        });
        let sys = SclParser::to_system_config(&ied_config);
        assert_eq!(sys.goose.dst_mac, "01:0C:CD:01:00:10");
        assert_eq!(sys.goose.appid, 0x0010);
    }
}
