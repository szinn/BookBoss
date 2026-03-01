# Development Setup

## Prerequisites

- Rust (nightly toolchain for formatting and clippy; `rust-version = "1.85"` edition 2024)
- [mise](https://mise.jdx.dev) — tool version manager
- [just](https://just.systems) — command runner
- [direnv](https://direnv.net/) — environment setup
- sops — secrets for environment

## One-time Setup

Install and update all required tools:

```bash
just install-tools
```

This installs the nightly toolchain, cargo extensions, and any other project tools.

## Building

```bash
just build
```

## Running

```bash
just run
```

## Secrets Configuration

```bash
just config
```

## Integration Tests

Integration tests require Docker via [Colima](https://github.com/abiosoft/colima):

```bash
colima start
just test
colima stop
```
