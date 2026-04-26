# Block-Tap Registry

Cardano protocol address registry and reusable module templates for [block-tap](https://github.com/defrag-au/block-tap).

## Structure

```
mainnet/
  dex/              # DEX protocols (Minswap, Splash, CSWAP, etc.)
  marketplace/      # NFT marketplaces (JPG.store, Wayup, etc.)
  defi/             # DeFi protocols (CrowdLock, etc.)
  staking/          # Staking contracts (The Vault, etc.)
modules/
  examples/         # Reusable module templates
```

## Components

Each TOML file in the network directories is a **component** describing a Cardano protocol:

```toml
[component]
name = "dex.minswap"
description = "Minswap V2 DEX — batcher, pool, and order contracts"
category = "dex"
network = "mainnet"

[component.match]
addresses = ["addr1w8p79rpkcdz8x9d6tft0x0dx5mwuzac2sa4gm8cvkw5hcnqst2ctf"]
address_prefixes = [
    "addr1z84q0denmyep98ph3tmzwsmw0j7zau9ljmsqx6a4rvaau6",
]
has_redeemers = true

[component.qualify]
resolve_inputs = true
detectable_types = ["swap", "liquidity_add", "liquidity_remove"]
```

### Fields

**component** — metadata
- `name` — unique identifier in `category.protocol` format
- `description` — human-readable description
- `category` — address group this belongs to (`dex`, `marketplace`, `defi`, `staking`)
- `network` — Cardano network (`mainnet`, `preprod`, `preview`)

**component.match** — structural patterns for fast filtering
- `addresses` — exact script addresses
- `address_prefixes` — address prefixes for contracts with per-pool/per-user staking credentials
- `has_redeemers` — whether transactions require Plutus script execution

**component.qualify** — indexer-assisted classification capabilities
- `resolve_inputs` — whether input resolution is needed for classification
- `detectable_types` — transaction types the classifier can identify for this protocol

## Address Groups

Components in a category form an **address group**. Block-tap modules can reference groups like `dex` or `marketplace` instead of listing individual addresses. The group resolves at runtime to all addresses and prefixes from all components in that category.

```toml
# Module using an address group
[[routes]]
name = "dex-swaps"
match = [
    { type = "address_group", group = "dex" },
    { type = "has_redeemers" },
]
destination = { type = "log" }
```

When a new protocol is added to the registry, all modules using its address group automatically include it.

## Module Templates

The `modules/examples/` directory contains reusable module templates:

- **dex-firehose.toml** — all DEX activity across known DEXes
- **token-dex-trades.toml** — DEX trades for a specific token (buys and sells)
- **marketplace-sales.toml** — NFT marketplace sales for a specific policy
- **policy-mints.toml** — mint tracking for a specific policy

Templates use `YOUR_POLICY_ID_HERE` as a placeholder. Replace with your policy ID before uploading.

## Contributing

To add a new protocol:

1. Create a TOML file in the appropriate `mainnet/<category>/` directory
2. Include all known script addresses and address prefixes
3. Set `has_redeemers` if the protocol uses Plutus scripts
4. List detectable transaction types in `component.qualify`
5. Open a PR

The format is designed to be human and LLM-readable. Block-tap's MCP tooling can read these files directly to help compose modules.
