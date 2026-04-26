//! Registry validator — validates all component TOML files in the registry.
//!
//! Run from the repo root:
//!   cargo run -p registry-validator
//!
//! Checks:
//! - All TOML files parse correctly
//! - Required fields are present and non-empty
//! - Addresses are valid bech32 format
//! - No duplicate component names
//! - Address prefixes are valid address starts
//! - Categories match directory names

use std::collections::HashSet;
use std::path::PathBuf;

use registry_resolver::load_local;

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_names = HashSet::new();
    let mut total_components = 0u32;
    let mut total_addresses = 0u32;
    let mut total_prefixes = 0u32;

    for network in &["mainnet", "preprod", "preview"] {
        let by_category = match load_local(&root, network) {
            Ok(c) => c,
            Err(e) => {
                // Network dir might not exist — that's fine
                if root.join(network).exists() {
                    errors.push(format!("[{network}] failed to load: {e}"));
                }
                continue;
            }
        };

        for (category, components) in &by_category {
            for component in components {
                let name = &component.component.name;
                total_components += 1;

                // Duplicate name check
                if !seen_names.insert(format!("{network}:{name}")) {
                    errors.push(format!("[{network}/{category}] duplicate component name: {name}"));
                }

                // Name format check
                let expected_prefix = format!("{category}.");
                if !name.starts_with(&expected_prefix) {
                    errors.push(format!(
                        "[{network}/{category}] component name '{name}' should start with '{expected_prefix}'"
                    ));
                }

                // Category matches directory
                if component.component.category != *category {
                    errors.push(format!(
                        "[{network}/{category}] component '{name}' has category '{}' but is in '{category}/' directory",
                        component.component.category
                    ));
                }

                // Network matches
                if component.component.network != *network {
                    errors.push(format!(
                        "[{network}/{category}] component '{name}' has network '{}' but is in '{network}/' directory",
                        component.component.network
                    ));
                }

                // Non-empty description
                if component.component.description.is_empty() {
                    warnings.push(format!("[{network}/{category}] component '{name}' has empty description"));
                }

                // Address validation
                let mc = &component.component.match_config;
                for addr in &mc.addresses {
                    total_addresses += 1;
                    if !is_valid_bech32_address(addr) {
                        errors.push(format!(
                            "[{network}/{category}/{name}] invalid address: {addr}"
                        ));
                    }
                }

                for prefix in &mc.address_prefixes {
                    total_prefixes += 1;
                    if !is_valid_address_prefix(prefix) {
                        errors.push(format!(
                            "[{network}/{category}/{name}] invalid address prefix: {prefix}"
                        ));
                    }
                }

                // Must have at least one address or prefix
                if mc.addresses.is_empty() && mc.address_prefixes.is_empty() {
                    errors.push(format!(
                        "[{network}/{category}/{name}] no addresses or prefixes defined"
                    ));
                }
            }
        }
    }

    // Print results
    println!("Registry Validation");
    println!("===================");
    println!("Components: {total_components}");
    println!("Addresses:  {total_addresses}");
    println!("Prefixes:   {total_prefixes}");
    println!();

    if !warnings.is_empty() {
        println!("Warnings ({}):", warnings.len());
        for w in &warnings {
            println!("  WARN  {w}");
        }
        println!();
    }

    if errors.is_empty() {
        println!("All checks passed.");
        Ok(())
    } else {
        println!("Errors ({}):", errors.len());
        for e in &errors {
            println!("  ERROR {e}");
        }
        std::process::exit(1);
    }
}

/// Basic bech32 address validation — starts with addr1 and is reasonable length.
fn is_valid_bech32_address(addr: &str) -> bool {
    (addr.starts_with("addr1") || addr.starts_with("addr_test1"))
        && addr.len() > 40
        && addr.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Address prefix validation — must be a valid address start.
fn is_valid_address_prefix(prefix: &str) -> bool {
    (prefix.starts_with("addr1") || prefix.starts_with("addr_test1"))
        && prefix.len() >= 10
        && prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
