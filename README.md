# BookBoss

[![Quillx](https://raw.githubusercontent.com/qainsights/Quillx/main/badges/quillx-3.svg)](https://github.com/qainsights/Quillx) This project is human-directed, AI co-authored, human-reviewed, and human-tested.

**Take control of your digital library.**

BookBoss is a self-hosted digital library manager built in Rust. It provides a
web-based interface for organising and browsing your e-book collection, backed
by a flexible database layer that supports PostgreSQL, MySQL, MariaDB, and SQLite.

## Features

### Library

- Browse your book library with cover art, title, author, series, and publisher
- View book detail pages with full metadata (description, genres, tags, language, publication date, identifiers)
- View author detail pages listing all their books
- View series detail pages listing all books in a series
- Edit book metadata (title, author, series, publisher, genres, tags, description, identifiers, cover)
- Delete books from the library

### Import

- Drop books into a watched folder — BookBoss scans and picks them up automatically
- Metadata extracted from EPUB files automatically
- Metadata enriched from external providers (Hardcover, OpenLibrary, GoogleBooks)
- Review incoming books before they enter the library (approve or reject)
- Cover art fetched automatically from metadata providers

### Shelves

- Create manual shelves and add/remove books
- Create smart shelves with filter rules (e.g. "all books with read status Active")
- Smart shelves update automatically as books and reading state change

### Reading State

- Track reading status per book (e.g. Active, Finished, etc.)
- Per-user — each user has their own reading state

### Kobo Device Sync

- Register Kobo devices to your account
- Each device gets a companion smart shelf — books on the shelf sync to the device
- Incremental sync — only sends new or changed books each time
- Cover art served to the Kobo automatically
- Download book files directly to the device (EPUB and KEPUB)
- Reset sync state to force a full re-sync
- Copy device sync URL to clipboard from the profile page
- Kobo-initiated book removal handled — deleted books re-sync on next connection

### User & Admin

- User registration and login
- Admin first-run setup
- Multi-user support (each user has their own reading state, shelves, devices)
- Settings page with library stats (total books, authors)
- User management (admin)

## Development Requirements

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

| Variable                             | Purpose                                               |
| ------------------------------------ | ----------------------------------------------------- |
| `BOOKBOSS__DATABASE__DATABASE_URL`   | SeaORM connection string (Postgres / MySQL / SQLite)  |
| `BOOKBOSS__LIBRARY__LIBRARY_PATH`    | Where approved book files are stored                  |
| `BOOKBOSS__IMPORT__BOOKDROP_PATH`    | Drop e-book files here to trigger the import pipeline |
| `BOOKBOSS__FRONTEND__LISTEN_IP`      | Server listen address (default `0.0.0.0`)             |
| `BOOKBOSS__FRONTEND__LISTEN_PORT`    | Server listen port (default `8080`)                   |
| `PGUSER`, `PGPASSWORD`, `PGDATABASE` | Used by `just create-database` and `just database`    |
| `PGADMINUSER`, `PGADMINPASSWORD`     | Admin credentials for database creation               |

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

The application will be available at `http://localhost:8080` by default. On
first launch you will be prompted to create an administrator account.

## Development

### Build

```bash
just build
```

### Common commands

| Command                           | Description                                         |
| --------------------------------- | --------------------------------------------------- |
| `just fmt`                        | Format code (Rust + Prettier)                       |
| `just clippy`                     | Run Clippy lints                                    |
| `just test`                       | Run all tests                                       |
| `just quick-test`                 | Component tests + Postgres/SQLite integration tests |
| `just component-tests`            | Unit/component tests only                           |
| `just integration-tests`          | All integration tests (requires Colima)             |
| `just postgres-integration-tests` | Postgres integration tests                          |
| `just sqlite-integration-tests`   | SQLite integration tests                            |
| `just mysql-integration-tests`    | MySQL integration tests                             |
| `just insta`                      | Run snapshot tests with cargo-insta                 |
| `just deps`                       | Update Rust crate dependencies                      |
| `just changelog`                  | Regenerate CHANGELOG.md                             |
| `just clean`                      | Clean the workspace                                 |

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
├── api/              # Adapter: gRPC interface
├── database/         # Adapter: SeaORM persistence (Postgres / MySQL / SQLite)
├── formats/          # Adapter: e-book file format support (EPUB, OPF)
├── frontend/         # Adapter: Dioxus web UI (fullstack)
├── import/           # Adapter: library scanner + import job handler
├── metadata/         # Adapter: external metadata providers
├── storage/          # Adapter: local filesystem library store
├── utils/            # Shared utilities
├── bookboss/         # Binary: wires adapters to ports
└── integration-tests/
```

See the [full documentation](docs/src/) for architecture details and contributor
guides.

## License

MIT
