<p align="center"><img src="assets/logo-256.png" width="128" alt="OxiScan logo"></p>

# OxiScan

[![CI](https://github.com/schms27/oxiscan/actions/workflows/ci.yml/badge.svg)](https://github.com/schms27/oxiscan/actions/workflows/ci.yml)

A fast, lightweight network IP scanner written in Rust, with a native GUI built on [egui](https://github.com/emilk/egui). It ships as a single executable, with no external dependencies and no installer.

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

## License

[MIT](LICENSE)
