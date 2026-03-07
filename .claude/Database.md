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

## SeaORM Adapter Patterns

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
