<p align="center">
  <img src="assets/logo.svg" alt="OxiScan logo" width="112" height="112">
</p>

<h1 align="center">OxiScan</h1>

<p align="center">
  <strong>A fast, self-contained network IP scanner for Windows, Linux and macOS.</strong>
</p>

<p align="center">
  <a href="https://github.com/schms27/oxiscan/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://github.com/schms27/oxiscan/actions/workflows/ci.yml/badge.svg?branch=main" alt="Build status on main"></a>
  <a href="https://github.com/schms27/oxiscan/releases/latest"><img src="https://badgen.net/github/release/schms27/oxiscan" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
  <a href="https://ko-fi.com/schms27"><img src="https://img.shields.io/badge/Ko--fi-support%20the%20project-FF5E5B?logo=ko-fi&logoColor=white" alt="Support on Ko-fi"></a>
</p>

OxiScan is written in Rust, with a native GUI built on [egui](https://github.com/emilk/egui). It ships as a single executable, with no external dependencies and no installer.

## Features

- Scan an IP range, random addresses, or a list of addresses read from a file
- Per host: ping time, host name, and open ports
- Optional columns: TTL, MAC address, Windows (NetBIOS) name, and web server type
- Sort, filter, rescan, and export the results
- Open a host in your browser, in a ping window, or over remote desktop
- Save favorite ranges

## Building

You need a recent stable Rust toolchain.

```sh
cargo build --release
```

The executable is written to `target/release/oxiscan` (`oxiscan.exe` on Windows).

Prebuilt binaries for Windows, Linux and macOS are attached to each [GitHub release](https://github.com/schms27/oxiscan/releases). To cut a release, push a version tag:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

## Status

This is an early first version, developed and tested on Windows. Linux and macOS builds are possible but have not been tested much.

## Background

OxiScan is a clean-room reimplementation inspired by the feature set of [Angry IP Scanner](https://angryip.org/). It shares no code with that project and is not affiliated with or endorsed by it.

## Support

OxiScan is free and open source. If it's useful to you, you can [buy me a coffee on Ko-fi](https://ko-fi.com/schms27). It's a thank-you, not a support contract: issues and pull requests are welcome either way.

## License

[MIT](LICENSE)
