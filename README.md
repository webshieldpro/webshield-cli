# webshield — WebShield CLI

Command-line client for managing domains, DNS records and publishing static sites via [WebShield](https://webshield.pro). 
For detailed API instructions and usage guides, check out the [WebShield Documentation](https://docs.webshield.pro).


## Installation

Downloads the binary for your OS/architecture, verifies the checksum, installs to ~/.local/bin
```sh
curl -fsSL https://raw.githubusercontent.com/webshieldpro/webshield-cli/main/install.sh | sh
```

## Building from source

```sh
cargo build --release
mv target/release/webshield ~/.local/bin/
```

Run the test suite (unit tests with a mock API server + black-box tests of the binary):

```sh
cargo test
```

Shell completion
```sh
# bash (or system-wide: /etc/bash_completion.d/webshield)
webshield completion bash > ~/.local/share/bash-completion/completions/webshield

# zsh (make sure the target dir is on $fpath and `compinit` is loaded)
webshield completion zsh > ~/.zfunc/_webshield

# fish
webshield completion fish > ~/.config/fish/completions/webshield.fish

# PowerShell (append to your profile)
webshield completion powershell >> $PROFILE

# elvish (append to your rc.elv)
webshield completion elvish >> ~/.config/elvish/rc.elv

# nushell — save the script, then `source` it from your config.nu
webshield completion nushell > ~/.config/nushell/completions/webshield.nu
# then add to config.nu:  source ~/.config/nushell/completions/webshield.nu
```

## Authentication

The client uses a personal `wsk_…` token (created in the control panel under
"Settings/API tokens", with the scopes you need and, optionally, bound to a domain/site).

```sh
webshield auth login --token wsk_...        # saves the token into a profile
webshield auth status                       # verifies access
```

The token can also be passed via `--token` or the `WS_TOKEN` env variable. Profiles are stored in
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
webshield dns add example.com www CNAME foo.example.com  # trailing dot added automatically
webshield dns dnssec enable example.com

# Static sites
webshield sites list
webshield sites create www.example.com --domain example.com
webshield sites publish www.example.com --dir ./public       # incremental
webshield sites publish --site-id 6 --dir ./public           # by id (narrow sites:publish tokens)
webshield sites publish www.example.com --dir ./public --dry-run

# Proxy/redirect hosts (edge settings)
webshield proxy list
webshield proxy set app.example.com --domain example.com --bot-protection true --ssl true
webshield proxy set old.example.com --domain example.com \
    --mode redirect --redirect-target example.com
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

DNS semantics match the backend: for A/AAAA/TXT/MX `add` extends the set
(it does not overwrite existing values), `set` reconciles the set to exactly
the given values, `remove` without values deletes the whole rrset.
