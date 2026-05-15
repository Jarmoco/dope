# DOPE

DOPE is a **Man-in-the-Middle proxy** that **injects userscripts** ([ViolentMonkey](https://violentmonkey.github.io/) / [GreaseMonkey](https://www.greasespot.net/) format) into web pages.
It can intercept HTTP and HTTPS traffic (with a custom CA), tweak headers in requests and responses, and comes with a web admin panel (optional).

## Quick Start

Download a binary from the [releases](https://github.com/Jarmoco/dope/releases/latest) page, or build from source with `./rcc-scripts/build.sh`.

```bash
# 1. Generate a CA certificate
openssl req -x509 -newkey rsa:4096 -keyout ca/ca.key -out ca/ca.cer -days 365 -nodes

# 2. Create a scripts folder and add your userscripts (*.user.js)
mkdir -p scripts

# 3. Start the proxy (default port 8080)
dope

# 4. (Optional) Start the web admin panel
dope-panel
```

Now configure your browser to use the proxy at `http://localhost:8080`. The proxy will generate a default `config.toml` on first run.

## CLI Usage

### `dope` — MITM proxy

```
dope [OPTIONS]

  -h, --help            Print this help message
  -pp                   Pretty-print logs (no timestamps, no targets)
  --scripts <path>      Userscript directory           [default: scripts]
  --logs <path>         Log output directory           [default: logs]
  --ca <path>           CA certificate directory       [default: ca]
  --config <path>       Configuration file path        [default: config.toml]

RUST_LOG            Log level (trace, debug, info, warn, error)
                    Default: info
```

### `dope-panel` — Web admin panel

```
dope-panel [OPTIONS]

  -h, --help            Print this help message
  --scripts <path>      Userscript directory           [default: scripts]
  --logs <path>         Log output directory           [default: logs]
  --config <path>       Configuration file path        [default: config.toml]
```

The panel serves an HTMX-powered UI on `http://127.0.0.1:9090` to view logs and manage configuration.

## Default Layout

```
dope/
├── dope-x.y.z-os-arch (binary)
├── dope-panel-x.y.z-os-arch (binary, optional)
├── ca/
│   ├── ca.key
│   └── ca.cer
├── scripts/
│   └── example.user.js
├── logs/
│   ├── dope.log
│   └── dope-traces.jsonl
└── config.toml
```

All paths default to the values shown above and can be overridden with the CLI flags listed in the previous section.

## Configuration

The proxy generates a `config.toml` on first launch. You can use the web panel to manage it, or edit it manually.
Inside the config file, you can define:

- **Script injection rules** — which userscripts run on which domains
- **Response modifiers** — CSP handling, header injection/removal, script injection position
- **Request modifiers** — outgoing request header manipulation

See the inline comments in the generated config file for details.

## GM\* Functions

The proxy automatically injects a Greasemonkey API polyfill when userscripts with `GM_` calls are detected. This allows userscripts to use functions like `GM_getValue`, `GM_setValue`, `GM_listValues`, `GM_deleteValue`, and `GM_xmlhttpRequest`.
