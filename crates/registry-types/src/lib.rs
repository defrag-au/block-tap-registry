//! Type definitions for the block-tap Cardano address registry.
//!
//! These types are the canonical representation of component TOML files
//! in the registry. Shared by block-tap, the MCP tool, and the registry
//! validator.

use serde::{Deserialize, Serialize};

/// A parsed component file — the top-level TOML wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub component: ComponentMeta,
}

impl Component {
    /// Parse a component from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

/// Component metadata and configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMeta {
    /// Unique name in `category.protocol` format (e.g., "dex.minswap").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Address group category (dex, marketplace, defi, staking).
    pub category: String,
    /// Cardano network (mainnet, preprod, preview).
    pub network: String,
    /// Structural match configuration.
    #[serde(rename = "match")]
    pub match_config: ComponentMatch,
    /// Optional qualification configuration.
    #[serde(default)]
    pub qualify: Option<ComponentQualify>,
}

/// Structural match configuration — addresses and patterns for fast filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMatch {
    /// Exact script addresses.
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Address prefixes for contracts with per-pool/per-user staking credentials.
    #[serde(default)]
    pub address_prefixes: Vec<String>,
    /// Whether transactions must have Plutus script redeemers.
    #[serde(default)]
    pub has_redeemers: Option<bool>,
    /// Whether transactions must have mint/burn activity.
    #[serde(default)]
    pub has_mint: Option<bool>,
}

/// Qualification configuration — what the indexer + classifier can determine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentQualify {
    /// Whether input UTxO resolution is needed for classification.
    #[serde(default)]
    pub resolve_inputs: bool,
    /// Transaction types this component's classifier can detect.
    #[serde(default)]
    pub detectable_types: Vec<String>,
}

/// A resolved address group — the merged addresses and prefixes from all
/// components in a category. This is what the worker uses at runtime.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressGroup {
    /// Group name (category), e.g., "dex".
    pub name: String,
    /// Network this group applies to.
    pub network: String,
    /// All exact addresses from all components in this group.
    pub addresses: Vec<String>,
    /// All address prefixes from all components in this group.
    pub prefixes: Vec<String>,
    /// All component names that contributed to this group.
    pub components: Vec<String>,
}

impl AddressGroup {
    /// Check if a given address matches this group (exact or prefix).
    pub fn matches_address(&self, addr: &str) -> bool {
        self.addresses.contains(&addr.to_string())
            || self.prefixes.iter().any(|p| addr.starts_with(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_component_toml() {
        let toml = r#"
            [component]
            name = "dex.minswap"
            description = "Minswap V2 DEX"
            category = "dex"
            network = "mainnet"

            [component.match]
            addresses = ["addr1w8test"]
            address_prefixes = ["addr1z84q0"]
            has_redeemers = true

            [component.qualify]
            resolve_inputs = true
            detectable_types = ["swap", "liquidity_add"]
        "#;

        let component = Component::from_toml(toml).unwrap();
        assert_eq!(component.component.name, "dex.minswap");
        assert_eq!(component.component.category, "dex");
        assert_eq!(component.component.match_config.addresses.len(), 1);
        assert_eq!(component.component.match_config.address_prefixes.len(), 1);
        assert!(component.component.match_config.has_redeemers.unwrap());
        assert_eq!(
            component.component.qualify.unwrap().detectable_types,
            vec!["swap", "liquidity_add"]
        );
    }

    #[test]
    fn address_group_matching() {
        let group = AddressGroup {
            name: "dex".to_string(),
            network: "mainnet".to_string(),
            addresses: vec!["addr1w8exact".to_string()],
            prefixes: vec!["addr1z84q0".to_string()],
            components: vec!["dex.minswap".to_string()],
        };

        assert!(group.matches_address("addr1w8exact"));
        assert!(group.matches_address("addr1z84q0denmyep98xyz"));
        assert!(!group.matches_address("addr1qy_something_else"));
    }
}
