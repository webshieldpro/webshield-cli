# WebShield CLI

Domains, DNS records, static sites. The terminal handles all of it — zero trips through the web dashboard, zero waiting for pages to load. Deploy, tweak, keep an eye on things. And the dashboard itself? Give it a week and you'll honestly forget it even has a login button.

Deep dives, weird edge cases, the full map — that lives in the [WebShield Docs](https://docs.webshield.pro).

## Installation

Two lines of shell. That's the entire install, seriously.

    curl -fsSL https://raw.githubusercontent.com/webshieldpro/webshield-cli/main/install.sh | sh

The script sniffs out your OS and architecture, grabs the right binary, checks the checksums — nothing fishy sneaks through — and drops it into `~/.local/bin`. And you're done, no package-manager wars, no sudo prompts.

## Building from source

Rust. Mandatory, no way around it — it's a Rust project, after all.

    cargo build --release
    mv target/release/webshield ~/.local/bin/

Tests? They exist too, for the suspicious types (and you should be suspicious):

    cargo test

## Shell completion

Tab completion is one of *those* things. You live fine without it for years, then you try it once — and going back feels like typing with mittens on.

    # bash — system-wide: /etc/bash_completion.d/webshield
    webshield completion bash > ~/.local/share/bash-completion/completions/webshield

    # zsh (the target directory has to sit on your $fpath, and compinit must be loaded)
    webshield completion zsh > ~/.zfunc/_webshield

    # fish
    webshield completion fish > ~/.config/fish/completions/webshield.fish

    # PowerShell (appends to your profile script)
    webshield completion powershell >> $PROFILE

    # elvish (lands straight in rc.elv)
    webshield completion elvish >> ~/.config/elvish/rc.elv

    # nushell — write to a file first, then pull it into config.nu via `source`
    webshield completion nushell > ~/.config/nushell/completions/webshield.nu
    # afterwards add this line to config.nu: source ~/.config/nushell/completions/webshield.nu

## Authentication

First stop: **Settings → API tokens** in the dashboard. That's where tokens get minted, and every one of them starts with `wsk_` — you'll spot them in the wild.

Here's the part I like: scoping. One token → just a couple of features. Or glued to a single domain. Future you will be grateful.

    webshield auth login --token wsk_...        # saves the token into a profile
    webshield auth status                       # quick ping, confirms the plumbing works

Don't feel like storing secrets on disk? Fair enough. Pass `--token` on the fly, or export `WS_TOKEN`. Your call.

Every profile sits in `~/.config/webshield/config.toml`. Running several? `--profile` picks one.

## Examples

Copy-paste territory. Swap the names, you know the drill.

    # Managing domains
    webshield domains add example.com --import scan

    # DNS records
    webshield dns add example.com www A 203.0.113.10      # appends an extra value
    webshield dns set example.com @ A 203.0.113.10        # overwrites and sets exactly this value
    webshield dns remove example.com www A 203.0.113.10   # removes just this single value
    webshield dns add example.com www CNAME foo.example.com  # the trailing dot gets appended automatically

    # Deploying static sites
    webshield sites create www.example.com --domain example.com
    webshield sites publish www.example.com --dir ./public       # uploads only new/modified assets
    webshield sites publish --site-id 6 --dir ./public           # handy if the API token only sees this specific site ID

    # Proxy configs and redirects
    webshield proxy set app.example.com --domain example.com --bot-protection true --ssl true
    webshield proxy set old.example.com --domain example.com \
        --mode redirect --redirect-target example.com

    # Analytics
    webshield stats summary example.com --range 7d

    # Pipe the output straight to JSON
    webshield -o json domains list 1

### A note on `add` / `set` / `remove` logic (A, AAAA, TXT, MX)

Three tiny words, three completely different moods. Simple, they are not — read carefully:

- `add` — tacks a value on. Whatever sat there before stays put.
- `set` — flattens the record and installs exactly what you typed. No leftovers.
- `remove` with no value — poof. Whole record, gone.
