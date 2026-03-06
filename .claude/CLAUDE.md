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
├── auth/               # Session auth: Session, AuthService, SessionRepository
├── book/               # Books, authors, series, publishers, genres, tags, files
├── device/             # Device sync: Device, DeviceBook, DeviceSyncLog
├── import/             # Acquisition pipeline: ImportJob, ImportJobService
├── pipeline/           # Port traits: MetadataExtractor, MetadataProvider
├── reading/            # Per-user reading state: UserBookMetadata, ReadStatus
├── shelf/              # Shelves (manual + smart): Shelf, ShelfFilter
├── storage/            # LibraryStore port trait + BookSidecar struct
└── user/               # Users and settings: User, UserService, UserSettingService
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

### Subsystem Pattern (tokio-graceful-shutdown)

Each crate that owns background work exposes a `XxxSubsystem` struct + `create_xxx_subsystem()` factory
in its `lib.rs` — same pattern as `ApiSubsystem` in `bb-api`. The subsystem's `run()` starts its
child subsystems via `subsys.start(SubsystemBuilder::new(...))` then awaits `on_shutdown_requested()`.
`bookboss/main.rs` stays clean: build any shared state (e.g. `JobRegistry`), call the factories,
pass results to `Toplevel`. Existing subsystems: `ApiSubsystem` (bb-api), `CoreSubsystem` (bb-core,
owns `JobWorker`), `ImportSubsystem` (bb-import, owns `LibraryScanner`).

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

### SeaORM Adapter Patterns

**Enum storage:** All domain enums stored as plain `String` columns (no DB CHECK constraints).
Conversion functions are module-private (`book_status_to_str` / `str_to_book_status`).
`From<Model> for DomainType` is infallible and panics on unknown values — acceptable since all
writes go through adapters.

**`ActiveModelBehavior` / `before_save`:** The `books` entity has a `before_save` hook that
auto-increments `version` and sets `updated_at`. When inserting, use `version: Set(0)` — the
hook bumps it to 1. Don't fight it.

**Optimistic locking pattern:**

```rust
let existing = Entity::find_by_id(id).one(db_tx).await?.ok_or(NotFound)?;
if existing.version != record.version { return Err(VersionConflict); }
// set all mutable fields, then .update()
```

**Junction table filter (subquery pattern):**

```rust
use sea_orm::sea_query::Query;
if let Some(author_id) = filter.author_id {
    let mut subq = Query::select();
    subq.column(book_authors::Column::BookId)
        .from(book_authors::Entity)
        .and_where(book_authors::Column::AuthorId.eq(author_id as i64));
    query = query.filter(books::Column::Id.in_subquery(subq));
}
```

**Junction table inserts in tests:**

```rust
let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
book_authors::ActiveModel { book_id: Set(book.id as i64), ... }.insert(db_tx).await.unwrap();
```

**Adding a new repository to `RepositoryService`:**

1. Add field + accessor to `core/src/repository.rs` `RepositoryService`
2. Create `database/src/adapters/<name>.rs` with adapter impl + tests
3. Register in `database/src/adapters/mod.rs`
4. Import + wire into builder in `database/src/lib.rs`
5. Add `Mock<Name>Repository` to **4** test helpers:
   `core/src/auth/service.rs`, `core/src/book/service.rs`,
   `core/src/user/service/user.rs`, `core/src/user/service/user_settings.rs`

## Workflows

**After completing each task (end-of-task routine — run these as separate commands):**

1. `just fmt` — format code
2. `just clippy` — lint (run separately from fmt, not chained)
3. `just component-tests` — verify tests pass
4. `jj desc -m "type(scope): description\n\nbody"` — update working copy description
5. Update `.scratchpad/implementation-plan.md` — mark completed tasks `✓` / `[x]`, note partial work in later tasks

**Before committing (full pre-commit check):**

- Run all tests: `just test`
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
