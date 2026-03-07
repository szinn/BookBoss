# Architecture

BookBoss follows **hexagonal (ports & adapters)** architecture. Dependencies point inward toward the core domain. The `core` crate never depends on any outer crate.

## Crate Layout

```
crates/
├── api/            # Adapter: gRPC interface, calls into core ports
├── core/           # Domain layer: business logic, models, port traits
├── database/       # Adapter: persistence (SeaORM — Postgres, MySQL, SQLite)
├── formats/        # Adapter: e-book file format support (EPUB, OPF)
├── frontend/       # Adapter: Dioxus web UI, calls into core ports
├── metadata/       # Adapter: external metadata providers (Open Library, etc.)
├── storage/        # Adapter: local filesystem library store
├── utils/          # Shared utilities (token encoding, etc.)
├── bookboss/       # Entry point: wires adapters to ports
└── integration-tests/
```

Only `crates/bookboss` is a direct workspace member. All others are pulled in as path dependencies.

## Core Crate

The `core` crate uses domain-based modules. Each domain groups its model, repository trait (port), and service:

```
crates/core/src/
├── lib.rs              # CoreServices composition root, create_services()
├── error.rs            # Error, ErrorKind, RepositoryError
├── types.rs            # Shared newtypes (Email, Age)
├── repository.rs       # Repository, Transaction traits; RepositoryService; transaction macros
├── test_support.rs     # Mock implementations (behind "test-support" feature)
├── auth/               # Session auth: Session, AuthService, SessionRepository
├── book/               # Books, authors, series, publishers, genres, tags, files
├── device/             # Device sync: Device, DeviceBook, DeviceSyncLog
├── import/             # Acquisition pipeline: ImportJob, ImportJobService
├── jobs/               # Job queue: Job, JobRepository, JobWorker, JobRegistry, JobHandler
├── pipeline/           # Port traits: MetadataExtractor, MetadataProvider
├── reading/            # Per-user reading state: UserBookMetadata, ReadStatus
├── shelf/              # Shelves (manual + smart): Shelf, ShelfFilter
├── storage/            # LibraryStore port trait + BookSidecar struct
└── user/               # Users and settings: User, UserService, UserSettingService
```

Each domain module typically contains:

- `mod.rs` — re-exports
- `model.rs` (or `model/`) — domain types (`Foo`, `NewFoo`, `FooId`, `FooToken`)
- `repository.rs` (or `repository/`) — `FooRepository` trait (port)
- `service.rs` — `FooService` trait + `FooServiceImpl`

## Adding a New Domain

1. Create a directory under `crates/core/src/` (e.g. `book/`)
2. Add `mod.rs`, `model.rs`, `repository.rs`, `service.rs`
3. Re-export from `mod.rs`
4. Register the module in `lib.rs`
5. Wire the new service into `CoreServices`

## Import Conventions

Use flat re-exports from domain modules:

```rust
use crate::user::{User, UserService, UserId};       // not user::model::User
use crate::session::{Session, NewSession};
use crate::repository::{Repository, Transaction};
use crate::types::{Email, Age};
```

Cross-domain references are allowed (e.g. `use crate::user::UserId` in an order model for foreign keys). Keep references one-directional where possible.
