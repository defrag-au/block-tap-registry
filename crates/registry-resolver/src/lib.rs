//! Registry resolver — fetches and parses components from the block-tap registry.
//!
//! Supports two sources:
//! - **Local path** — reads TOML files from a local directory (for MCP tool, validator, tests)
//! - **GitHub** — fetches from the GitHub API (for runtime resolution in workers)
//!
//! The resolver produces `AddressGroup`s by merging all components in a category.

use std::collections::BTreeMap;
use std::path::Path;

use registry_types::{AddressGroup, Component};
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("failed to read component file: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("failed to parse component TOML: {path}: {source}")]
    TomlParse {
        path: String,
        source: toml::de::Error,
    },

    #[error("HTTP fetch failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("GitHub API error: {status} {body}")]
    GitHubApi { status: u16, body: String },

    #[error("no components found for group '{0}'")]
    GroupNotFound(String),
}

/// Registry configuration.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// GitHub owner/repo (e.g., "defrag-au/block-tap-registry").
    pub repo: String,
    /// Git branch to fetch from.
    pub branch: String,
    /// Network directory (e.g., "mainnet").
    pub network: String,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            repo: "defrag-au/block-tap-registry".to_string(),
            branch: "main".to_string(),
            network: "mainnet".to_string(),
        }
    }
}

// ── Local resolution ─────────────────────────────────────────────────────

/// Load all components from a local directory, grouped by category.
pub fn load_local(
    base_dir: &Path,
    network: &str,
) -> Result<BTreeMap<String, Vec<Component>>, ResolverError> {
    let network_dir = base_dir.join(network);
    let mut by_category: BTreeMap<String, Vec<Component>> = BTreeMap::new();

    if !network_dir.exists() {
        return Ok(by_category);
    }

    for category_entry in std::fs::read_dir(&network_dir)? {
        let category_entry = category_entry?;
        let category_path = category_entry.path();
        if !category_path.is_dir() {
            continue;
        }

        let category = category_entry
            .file_name()
            .to_string_lossy()
            .to_string();

        for file_entry in std::fs::read_dir(&category_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();
            if file_path.extension().is_none_or(|ext| ext != "toml") {
                continue;
            }

            let content = std::fs::read_to_string(&file_path)?;
            let component = Component::from_toml(&content).map_err(|e| ResolverError::TomlParse {
                path: file_path.display().to_string(),
                source: e,
            })?;

            by_category
                .entry(category.clone())
                .or_default()
                .push(component);
        }
    }

    Ok(by_category)
}

/// Resolve a single address group from local component files.
pub fn resolve_group_local(
    base_dir: &Path,
    network: &str,
    group: &str,
) -> Result<AddressGroup, ResolverError> {
    let category_dir = base_dir.join(network).join(group);
    if !category_dir.exists() {
        return Err(ResolverError::GroupNotFound(group.to_string()));
    }

    let mut components = Vec::new();
    for entry in std::fs::read_dir(&category_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let component = Component::from_toml(&content).map_err(|e| ResolverError::TomlParse {
            path: path.display().to_string(),
            source: e,
        })?;
        components.push(component);
    }

    if components.is_empty() {
        return Err(ResolverError::GroupNotFound(group.to_string()));
    }

    Ok(merge_components(group, network, &components))
}

/// Resolve all address groups from local component files.
pub fn resolve_all_groups_local(
    base_dir: &Path,
    network: &str,
) -> Result<BTreeMap<String, AddressGroup>, ResolverError> {
    let by_category = load_local(base_dir, network)?;
    let mut groups = BTreeMap::new();

    for (category, components) in &by_category {
        groups.insert(
            category.clone(),
            merge_components(category, network, components),
        );
    }

    Ok(groups)
}

// ── GitHub resolution ────────────────────────────────────────────────────

/// Fetch and resolve a single address group from GitHub.
pub async fn resolve_group_github(
    config: &RegistryConfig,
    group: &str,
) -> Result<AddressGroup, ResolverError> {
    let files = fetch_github_directory(config, group).await?;

    let mut components = Vec::new();
    for (path, content) in &files {
        let component = Component::from_toml(content).map_err(|e| ResolverError::TomlParse {
            path: path.clone(),
            source: e,
        })?;
        components.push(component);
    }

    if components.is_empty() {
        return Err(ResolverError::GroupNotFound(group.to_string()));
    }

    Ok(merge_components(group, &config.network, &components))
}

/// Fetch all TOML files in a category directory from GitHub.
async fn fetch_github_directory(
    config: &RegistryConfig,
    group: &str,
) -> Result<Vec<(String, String)>, ResolverError> {
    let api_url = format!(
        "https://api.github.com/repos/{}/contents/{}/{}",
        config.repo, config.network, group
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&api_url)
        .header("User-Agent", "block-tap-registry")
        .query(&[("ref", config.branch.as_str())])
        .send()
        .await?;

    let status = resp.status().as_u16();
    if status >= 400 {
        let body = resp.text().await.unwrap_or_default();
        return Err(ResolverError::GitHubApi { status, body });
    }

    let entries: Vec<GitHubContent> = resp.json().await?;
    let mut files = Vec::new();

    for entry in entries {
        if !entry.name.ends_with(".toml") {
            continue;
        }

        let Some(download_url) = entry.download_url else {
            continue;
        };

        debug!(file = %entry.name, "fetching component from github");
        let content = client
            .get(&download_url)
            .header("User-Agent", "block-tap-registry")
            .send()
            .await?
            .text()
            .await?;

        files.push((entry.name, content));
    }

    Ok(files)
}

#[derive(serde::Deserialize)]
struct GitHubContent {
    name: String,
    download_url: Option<String>,
}

// ── Shared logic ─────────────────────────────────────────────────────────

/// Merge multiple components into a single address group.
fn merge_components(group: &str, network: &str, components: &[Component]) -> AddressGroup {
    let mut addresses = Vec::new();
    let mut prefixes = Vec::new();
    let mut component_names = Vec::new();

    for c in components {
        component_names.push(c.component.name.clone());
        addresses.extend(c.component.match_config.addresses.clone());
        prefixes.extend(c.component.match_config.address_prefixes.clone());
    }

    addresses.sort_unstable();
    addresses.dedup();
    prefixes.sort_unstable();
    prefixes.dedup();

    debug!(
        group,
        components = component_names.len(),
        addresses = addresses.len(),
        prefixes = prefixes.len(),
        "resolved address group"
    );

    AddressGroup {
        name: group.to_string(),
        network: network.to_string(),
        addresses,
        prefixes,
        components: component_names,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn registry_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn load_local_components() {
        let root = registry_root();
        let by_category = load_local(&root, "mainnet").unwrap();
        assert!(by_category.contains_key("dex"));
        assert!(by_category.contains_key("marketplace"));
        assert!(by_category["dex"].len() >= 5);
    }

    #[test]
    fn resolve_dex_group() {
        let root = registry_root();
        let group = resolve_group_local(&root, "mainnet", "dex").unwrap();
        assert_eq!(group.name, "dex");
        assert!(!group.addresses.is_empty());
        assert!(!group.prefixes.is_empty());
        assert!(group.components.contains(&"dex.minswap".to_string()));
    }

    #[test]
    fn resolve_all_groups() {
        let root = registry_root();
        let groups = resolve_all_groups_local(&root, "mainnet").unwrap();
        assert!(groups.contains_key("dex"));
        assert!(groups.contains_key("marketplace"));
        assert!(groups.contains_key("defi"));
        assert!(groups.contains_key("staking"));
    }

    #[test]
    fn nonexistent_group_errors() {
        let root = registry_root();
        let result = resolve_group_local(&root, "mainnet", "nonexistent");
        assert!(result.is_err());
    }
}
