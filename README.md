# BookBoss

**Take control of your digital library.**

BookBoss is a self-hosted digital library manager built in Rust. It provides a
web-based interface for organising and browsing your e-book collection, backed
by a flexible database layer that supports PostgreSQL, MySQL, and SQLite.

## Features

- Web UI built with [Dioxus](https://dioxus.dev) (fullstack, WASM)
- Multi-database support: PostgreSQL, MySQL, SQLite
- E-book file format support
- User management with role-based access (including SuperAdmin)
- Session-based authentication

## Requirements

- [Rust](https://rustup.rs) 1.85+ (nightly toolchain for formatting/clippy)
- [mise](https://mise.jdx.dev) — manages tool versions
- [just](https://just.systems) — task runner
- Node.js 24+ (for Tailwind CSS, managed by mise)
- An existing PostgreSQL or MySQL instance (for those database backends)

## Getting Started

### 1. Install tools

```bash
just install-tools
```

This runs `mise install` and adds the `nightly` Rust toolchain and the
`wasm32-unknown-unknown` target.

### 2. Configure

```bash
just config
```

Edit the encrypted `config.sops.env` file. Required variables:

| Variable                             | Purpose                                              |
| ------------------------------------ | ---------------------------------------------------- |
| `BOOKBOSS__DATABASE__DATABASE_URL`   | SeaORM connection string (Postgres / MySQL / SQLite) |
| `BOOKBOSS__FRONTEND__LISTEN_IP`      | Server listen address (default `0.0.0.0`)            |
| `BOOKBOSS__FRONTEND__LISTEN_PORT`    | Server listen port (default `8080`)                  |
| `PGUSER`, `PGPASSWORD`, `PGDATABASE` | Used by `just create-database` and `just database`   |
| `PGADMINUSER`, `PGADMINPASSWORD`     | Admin credentials for database creation              |

Connection string formats:

```
postgres://user:password@host:port/database
mysql://user:password@host:port/database
sqlite:path/to/file.db
```

> Secrets are encrypted with [sops](https://github.com/getsops/sops) — never
> commit plaintext secrets.

### 3. Create the database

```bash
just create-database
```

### 4. Run

```bash
just run
```

The application will be available at `http://localhost:8080` by default.

## Development

### Build

```bash
just build
```

### Common commands

| Command                  | Description                                         |
| ------------------------ | --------------------------------------------------- |
| `just fmt`               | Format code (Rust + Prettier)                       |
| `just clippy`            | Run Clippy lints                                    |
| `just test`              | Run all tests                                       |
| `just quick-test`        | Component tests + Postgres/SQLite integration tests |
| `just component-tests`   | Unit/component tests only                           |
| `just integration-tests` | All integration tests (requires Colima)             |
| `just insta`             | Run snapshot tests with cargo-insta                 |
| `just deps`              | Update Rust crate dependencies                      |
| `just changelog`         | Regenerate CHANGELOG.md                             |
| `just clean`             | Clean the workspace                                 |

### Integration tests

Integration tests use Docker containers managed by [Colima](https://github.com/abiosoft/colima):

```bash
colima start
just integration-tests
colima stop
```

## Architecture

BookBoss follows **hexagonal (ports & adapters)** architecture. All dependencies
point inward toward the core domain — the core crate has no knowledge of
adapters.

```
crates/
├── core/             # Domain: business logic, models, port traits
├── database/         # Adapter: SeaORM persistence (Postgres / MySQL / SQLite)
├── formats/          # Adapter: e-book file format support
├── frontend/         # Adapter: Dioxus web UI
├── metadata/         # Adapter: external metadata providers (Open Library, etc.)
├── storage/          # Adapter: local filesystem library store
├── utils/            # Shared utilities
├── bookboss/         # Binary: wires adapters to ports
└── integration-tests/
```

## License

MIT
