# BookBoss: Take Control Of Your Digital Library

## One-time Setup

This project uses edition 2024 with `rust-version = "1.85"` and the nightly toolchain for
formatting and clippy. Extra tools include [mise](https://mise.jdx.dev) and
[just](https://just.systems). To install/update the tools:

```bash
just install-tools
```

## Commands

- Install tools: `just install-tools`
- Edit configuration: `just config`
- Build: `just build`
- Run the application: `just run`
- Format code: `just fmt`
- Update rust crate dependencies: `just deps`
- Update tailwindcss: `just tailwindcss`
- Run clippy: `just clippy`
- Run quick tests (component + postgres): `just quick-test`
- Run all tests: `just test`
- Run component tests: `just component-tests`
- Run all integration tests: `just integration-tests`
- Run Postgres integration tests: `just postgres-integration-tests`
- Run SQLite integration tests: `just sqlite-integration-tests`
- Run MySQL integration tests: `just mysql-integration-tests`
- Run insta tests: `just insta`
- Clean workspace: `just clean`
- Create changelog: `just changelog`
- Database admin: `just database`
- Create database: `just create-database`
- Redo all migrations: `just migrations`
- Extract database entities: `just entities`
- Start colima for integration tests or all tests: `colima start`
- Stop colima: `colima stop`

## Architecture

This project follows hexagonal (ports & adapters) architecture. Dependencies point inward
toward the core domain. Never introduce dependencies from `core` to outer crates.

```
crates/
├── api/                # Adapter: GRPC interface, calls into core ports
├── core/               # Domain layer: business logic, domain models, and port traits (interfaces)
├── database/           # Adapter: implements persistence ports defined in core (SeaORM/Postgres)
├── formats/            # Adapter: e-book file format support
├── frontend/           # Adapter: user interface, calls into core ports
├── bookboss/           # Application entry point, wires adapters to ports
└── integration-tests/  # Integration tests
```

Only `crates/bookboss` is a direct workspace member. The other crates are pulled in transitively
as path dependencies.

### Core Crate Organization

The core crate uses **domain-based modules** — each domain concept groups its model,
repository trait (port), and service together:

```
crates/core/src/
├── lib.rs              # CoreServices composition root, create_services()
├── error.rs            # Error, ErrorKind, RepositoryError
├── types.rs            # Shared newtypes (Email, Age) used across domains
├── repository.rs       # Shared infrastructure: Repository, Transaction traits,
│                       #   RepositoryService, and transaction macros
├── test_support.rs     # Mock implementations (behind "test-support" feature)
├── user/
│   ├── mod.rs          # Re-exports: User, UserService, UserRepository, etc.
│   ├── model.rs        # User, NewUser, PartialUserUpdate, UserId, UserToken
│   ├── repository.rs   # UserRepository trait (port)
│   └── service.rs      # UserService trait + UserServiceImpl
└── session/
    ├── mod.rs          # Re-exports: Session, SessionService, SessionRepository, etc.
    ├── model.rs        # Session, NewSession, SessionBuilder
    ├── repository.rs   # SessionRepository trait (port)
    └── service.rs      # SessionService trait + SessionServiceImpl
```

**Adding a new domain:** Create a new directory (e.g. `order/`) with `mod.rs`, `model.rs`,
`repository.rs`, and `service.rs`. Add re-exports in `mod.rs` and register the module in
`lib.rs`. Wire the new service into `CoreServices`.

**Import conventions:** Use flat re-exports from domain modules, not submodule paths:

- `use crate::user::{User, UserService, UserId}` (not `user::model::User`)
- `use crate::session::{Session, NewSession}` (not `session::model::Session`)
- `use crate::repository::{Repository, Transaction}` for shared infrastructure
- `use crate::types::{Email, Age}` for shared newtypes

**Cross-domain references:** Domain modules can import types from sibling domains
(e.g. `use crate::user::UserId` in an order model for foreign-key relationships).
Keep references one-directional when possible.

## Frontend

The frontend is built using Dioxus. See @.claude/Dioxus.md for more info.

## Database

The project SeaORM to provide database support. Postgres, MySQL and SQLite are all
supported. For Postgres and MySQL, an existing instance is required for
database-related commands. The following environment variables must be set:

- `PGUSER`, `PGPASSWORD`, `PGDATABASE` — used by `just create-database` and `just database`
- `PGADMINUSER`, `PGADMINPASSWORD` — admin credentials for database creation
- `BOOKBOSS__DATABASE__DATABASE_URL` — SeaORM connection string for migrations and entity generation
  - Postgres: `postgres://user:password@host:port/database`
  - MySQL: `mysql://user:password@host:port/database`
  - SQLite: `sqlite::path`

Secrets should be encrypted with `sops` and never committed.

## Workflows

**After completing work:**

- Use component tests to make sure everything passes: `just component-tests`

**Before committing:**

- Run tests to make sure everything passes: `just test`
- Run clippy for linting: `just clippy`
- Format code: `just fmt`
- Update the working copy description with `jj desc -m "..."` — do not ask about committing
- The description should include a conventional commit title and a body summarizing what was done

## Testing

- Colima is used to manage docker containers required for integration testing
- Use `cargo-nextest` as the test runner (`just test`)
- Use `cargo-insta` for snapshot testing (`just insta`) when asserting against larger or
  structured output; use regular assertions for simple value checks
- Tests live alongside source code in `#[cfg(test)]` modules

## Conventions

- **Commits:**
  - Follow conventional commits with crate-based scopes sorted: `type(scope): description`
  - Valid scopes: `api`, `cli`, `core`, `database` (match crate names)
  - Use `jj` (jujutsu) for version control, not `git`
  - Key commands: `jj commit`, `jj describe`, `jj new`, `jj log`, `jj status`
- **Error handling:**
  - Use `thiserror` for typed errors in library crates (`core`, `api`, `database`)
  - Use `anyhow` for ad-hoc errors in the binary crate (`cli`)
- **Secrets:**
  - Secrets should be encrypted with `sops`, never commit secrets
- **Dependencies:**
  - All crate dependencies must be defined in the root `Cargo.toml` under `[workspace.dependencies]`
  - Individual crates reference them with `crate-name.workspace = true`
  - In root `Cargo.toml`: version-only deps use inline format (`anyhow = "1.0.100"`), but deps
      with features or other options use section format:

```toml
[workspace.dependencies.uuid]
version = "1"
features = ["v4", "serde"]
```
