# webshield — WebShield CLI

Command-line client for managing domains, DNS records and publishing static
sites through the `/api/v1` API of [WebShield](https://webshield.pro).
Written in Rust (tokio + reqwest).

## Installation

```sh
# Downloads the binary for your OS/architecture, verifies the checksum, installs to ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/webshieldpro/webshield-cli/main/install.sh | sh
```

Without arguments the latest release is installed; a specific version —
`… | sh -s -- 0.1.0`. The release base URL and install directory are
configurable via env: `WEBSHIELD_CLI_BASE_URL`, `WEBSHIELD_CLI_BINDIR`.

## Building from source

```sh
cargo build --release        # binary: target/release/webshield
```

## Cutting a release

Cross-building all platforms — via `cargo-zigbuild` (zig as the C compiler,
no Docker and no per-target toolchains):

```sh
cargo install cargo-zigbuild && pip install ziglang   # once
./build-release.sh                    # → dist/ (tar.gz/zip + SHA256SUMS)
```

Without zig the script builds only the native target. Artifacts are named
`webshield-<version>-<target>.{tar.gz,zip}`; shell completions are bundled
into every archive.

Automatic publishing to a GitHub release — on push of a `v<version>` tag
(workflow `.github/workflows/release.yaml`). The tag version must match
`version` in `Cargo.toml`. Manual publishing — `gh release create v<version> dist/*`.

## Authentication

The client uses a personal `wsk_…` token (created in the control panel under
"API tokens", with the scopes you need and, optionally, bound to a domain/site).

```sh
webshield auth login --token wsk_...        # saves the token into a profile
webshield auth status                       # verifies access
```

The token can also be passed via `--token` or the `WS_TOKEN` env variable;
the base URL — via `--api-url` or `WS_API_URL`. Profiles are stored in
`~/.config/webshield/config.toml` (multiple profiles via `--profile`).

## Examples

```sh
# Domains
webshield domains list
webshield domains add example.com --import scan
webshield domains check example.com

# DNS records
webshield dns list example.com
webshield dns add example.com www A 203.0.113.10      # add a value
webshield dns set example.com @ A 203.0.113.10        # set exactly this value
webshield dns remove example.com www A 203.0.113.10   # remove a single value
webshield dns remove example.com www TXT              # remove the whole set
webshield dns dnssec enable example.com

# Static sites
webshield sites list
webshield sites create www.example.com --domain example.com
webshield sites publish www.example.com --dir ./public       # incremental
webshield sites publish www.example.com --dir ./public --dry-run

# Proxy/redirect hosts (edge settings)
webshield proxy list
webshield proxy set app.example.com --domain example.com --bot-protection true --ssl true
webshield proxy set old.example.com --domain example.com --mode redirect --redirect-target example.com
webshield proxy remove app.example.com

# Statistics and protection
webshield stats summary example.com --range 7d
webshield stats bans example.com

# Billing (read-only)
webshield billing balance
webshield billing usage example.com
webshield billing tariffs example.com

# Machine-readable output
webshield -o json domains list
```

## Interface language

English or Russian. Selection: `--lang ru|en` flag → env `WS_LANG` → system
locale (`LANG`/`LC_*`) → English by default. All runtime output (messages,
tables, hints) and the command list in `--help` are localized. Descriptions
of individual arguments in help are English (universal tokens).

```sh
webshield --lang ru domains list
WS_LANG=ru webshield billing balance
```

DNS semantics match the backend: for A/AAAA/TXT/MX `add` extends the set
(it does not overwrite existing values), `set` reconciles the set to exactly
the given values, `remove` without values deletes the whole rrset.

## Shell completion

```sh
webshield completion bash > /etc/bash_completion.d/webshield
webshield completion zsh  > ~/.zfunc/_webshield
```

## Not implemented

Managing API tokens and object S3 storage requires a JWT session
(email login) — currently out of the CLI's scope.

## License

[MIT](LICENSE)
