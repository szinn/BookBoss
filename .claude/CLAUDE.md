# BookBoss: Take Control Of Your Digital Library

## Version Control

This is a **jj (jujutsu) repo**. Never use git commands (including `git worktree`).
Use only `jj` commands for all version control operations.

## Commands

- Build: `just build`
- Run: `just run`
- Format: `just fmt`
- Lint: `just clippy`
- Quick tests (component + postgres): `just quick-test`
- All tests: `just test`
- Component tests: `just component-tests`
- Integration tests: `just integration-tests`
- Postgres integration tests: `just postgres-integration-tests`
- SQLite integration tests: `just sqlite-integration-tests`
- MySQL integration tests: `just mysql-integration-tests`
- MariaDB integration tests: `just mariadb-integration-tests`
- Insta tests: `just insta`
- Start colima (for integration/all tests): `colima start`
- Stop colima: `colima stop`

## Architecture

This project follows hexagonal (ports & adapters) architecture. Dependencies point inward
toward the core domain. Never introduce dependencies from `core` to outer crates.

```
crates/
├── api/                # Adapter: GRPC interface, calls into core ports
├── core/               # Domain layer: business logic, domain models, and port traits (interfaces)
├── database/           # Adapter: implements persistence ports defined in core (SeaORM: Postgres, MySQL, SQLite)
├── formats/            # Adapter: e-book file format support (OPF, EPUB)
├── frontend/           # Adapter: user interface, calls into core ports
├── metadata/           # Adapter: MetadataProvider implementations (Hardcover, OpenLibrary)
├── storage/            # Adapter: local filesystem FileStoreService implementation
├── utils/              # Shared utilities: hashing, token generation
├── bookboss/           # Application entry point, wires adapters to ports
└── integration-tests/  # Integration tests
```

### Core Crate Organization

The core crate uses **domain-based modules** — each domain concept groups its model,
repository trait (port), and service together:

```
crates/core/src/
├── lib.rs              # CoreServices composition root, create_services()
├── error.rs            # Error, ErrorKind, RepositoryError
├── types/              # Shared newtypes (Email, Age) used across domains
├── repository.rs       # Shared infrastructure: Repository, Transaction traits,
│                       #   RepositoryService, and transaction macros
├── test_support.rs     # Mock implementations (behind "test-support" feature)
└── <domain>/           # One directory per domain, e.g. auth/, book/, import/,
                        #   library/, shelf/, user/, health/, jobs/, opds/
```

**Adding a new domain:** Create a new directory (e.g. `order/`) with `mod.rs`, `model.rs`,
`repository.rs`, and `service.rs`. Add re-exports in `mod.rs` and register the module in
`lib.rs`. Wire the new service into `CoreServices`.

**Import conventions:** Use flat re-exports from domain modules, not submodule paths:

- `use crate::user::{User, UserService, UserId}` (not `user::model::User`)
- `use crate::auth::{Session, NewSession}` (not `auth::model::Session`)
- `use crate::repository::{Repository, Transaction}` for shared infrastructure
- `use crate::types::{Email, Age}` for shared newtypes

### Subsystem Pattern (tokio-graceful-shutdown)

Each crate that owns background work exposes a `XxxSubsystem` struct + `create_xxx_subsystem()` factory
in its `lib.rs` — same pattern as `ApiSubsystem` in `bb-api`. The subsystem's `run()` starts its
child subsystems via `subsys.start(SubsystemBuilder::new(...))` then awaits `on_shutdown_requested()`.
`crates/bookboss/src/bin/bookboss.rs` stays clean: call the factories,
pass results to `Toplevel`. Existing subsystems: `ApiSubsystem` (bb-api), `CoreSubsystem` (bb-core,
starts the health-check, bookdrop-scan, and job-worker subsystems).

## Frontend

The frontend is built using Dioxus. See @Dioxus.md for more info.

## Database

The project uses SeaORM with Postgres, MySQL, and SQLite support. See @Database.md
for environment variable setup and SeaORM adapter patterns.

## Workflows

**Multi-step implementations:** Give each logical step its own jj changeset. Before
starting a step, ensure the working copy is empty (`jj new` if needed). At the end of each
step, run the end-of-task routine below.

**After completing each task (end-of-task routine):**

Run each as its own Bash command rather than chaining them with `&&`.

1. `just fmt-lint` — format code
2. `just component-tests` — verify tests pass
3. `jj desc -m "type(scope): description\n\nbody"` — update working copy description

### Workspaces

When creating a new workspace, run

```bash
direnv allow
mise trust
just tailwindcss
```

To verify the baseline state, run `just component-tests`.

## Testing

- Tests live alongside source code in `#[cfg(test)]` modules
- Colima manages docker containers required for integration testing

## Conventions

- **Commits:** Scope is the crate name (`api`, `bookboss`, `core`, `database`, `formats`,
  `frontend`, `metadata`, `storage`, `utils`); use `deps`, `ci`, or `release` for non-crate changes
- **Error handling:** `thiserror` for `core`, `api`, `database`; `anyhow` for `bookboss` (binary)

## Insights

This project uses `.insights/` for research, triage docs, specs, plans, and personal notes
managed by the `insights` CLI.

**At the start of brainstorming, spec writing, or planning work**, dispatch the
`insights-locator` agent to check for prior context before proceeding. Use
`insights-analyzer` to read the most relevant documents. Use the `insights-research`
skill to orchestrate both and save a research document.

Directory layout:

- `.insights/issues/` — triage documents (BB-XX-triage-\*.md)
- `.insights/shared/specs/` — specs (BB-XX-spec-\*.md)
- `.insights/shared/plans/` — plans (BB-XX-plan-\*.md)
- `.insights/shared/research/` — research documents
- `.insights/scotte/` — personal notes
- `.insights/searchable/` — hardlink mirror for grep/search (read-only; strip "searchable/"
  from any path before reporting or editing)
  All `.insights/` artifact files must include YAML front-matter.
  See `.insights/shared/schema.md` for the full schema and vocabulary.
