# DOPE

DOPE is a **Man-in-the-Middle proxy** that **injects userscripts** ([ViolentMonkey](https://violentmonkey.github.io/) / [GreaseMonkey](https://www.greasespot.net/) format) into web pages. It is designed to be used as a local proxy server that intercepts HTTP and HTTPS traffic, and injects userscripts into the responses.

## Quick Start

- Download a binary from the [releases](https://github.com/Jarmoco/dope/releases/latest) page, or build it from source.
- Generate the certificate files (see below) and add them to your browser's trusted certificates.
- Create a `scripts` folder in the same directory as the binary, and place your userscripts there. Make sure that they end in `.user.js`. ( You can use the example script [here](https://github.com/Jarmoco/dope/raw/refs/heads/main/scripts/example.user.js) )
- Launch the binary to let it generate the configuration file, then close it.
- Open `config.toml` and add entries for your userscripts. (Some are already there, just copy and edit the `website` and `scripts` fields)
- Launch the binary again to start the proxy.

Now you can configure your browser to use the proxy at `http://localhost:8080`. (Default configuration) and if you visit a website that matches the pattern in the configuration file, the userscript will be injected.

The folder structure should look like this:

```
dope/
├── dope-x.y.z-os-arch (binary)
├── ca/
│   ├── ca.key
│   └── ca.cer
├── scripts/
│   └── example.user.js
└── config.toml
```

### Certificate Generation

To generate the certificate files, run the following command:

```bash
openssl req -x509 -newkey rsa:4096 -keyout ca.key -out ca.cer -days 365 -nodes
```

This will generate two files: `ca.key` and `ca.cer`. Place them in the `ca` directory relative to the binary.

## GM_* Functions

The proxy automatically injects a Greasemonkey API polyfill when userscripts with `GM_` calls are detected. This allows userscripts to use functions like `GM_getValue`, `GM_setValue`, `GM_listValues`, `GM_deleteValue`, and `GM_xmlhttpRequest`.