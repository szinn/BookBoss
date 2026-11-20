# Database

The project uses SeaORM with Postgres, MySQL, and SQLite support. For Postgres and MySQL,
an existing instance is required for database-related commands.

## Environment Variables

- `PGUSER`, `PGPASSWORD`, `PGDATABASE` — used by `just create-database` and `just database`
- `PGADMINUSER`, `PGADMINPASSWORD` — admin credentials for database creation
- `BOOKBOSS__DATABASE__DATABASE_URL` — SeaORM connection string for migrations and entity generation
  - Postgres: `postgres://user:password@host:port/database`
  - MySQL: `mysql://user:password@host:port/database`
  - SQLite: `sqlite::path`

## SQLite Notes

When using the SQLite backend, BookBoss automatically configures these PRAGMAs
on every pool connection (see `bb_database::open_database`):

- `journal_mode=WAL` — readers and writers don't block each other
- `busy_timeout=5000` — wait up to 5s for a held lock before failing
- `synchronous=NORMAL` — durable across crashes (the standard pairing with WAL)
- `foreign_keys=true` — enforce FK constraints (off by default in SQLite)

**WAL sidecar files:** WAL mode produces two sidecar files alongside the main
database: `<db>-wal` (write-ahead log) and `<db>-shm` (shared memory index).
When backing up the SQLite database or bind-mounting it into a container,
include both sidecars — copying only the main `.sqlite` file mid-write can
produce a corrupt or out-of-date snapshot.

## SeaORM Adapter Patterns

**Migrations:** Only `up()` migrations need to be implemented. The `down()` method can just
be empty.

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
5. Add `Mock<Name>Repository` to every test helper that builds a full `RepositoryService`
   (search `crates/core/src` for `Mock…Repository::new` to find them)
