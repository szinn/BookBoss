# Database Internals

BookBoss uses [SeaORM](https://www.sea-ql.org/SeaORM/) for database access. PostgreSQL, MySQL, and SQLite are all supported.

## Environment Variables

The following environment variables are used by database-related `just` commands:

| Variable                           | Used by                                 |
| ---------------------------------- | --------------------------------------- |
| `PGUSER`                           | `just create-database`, `just database` |
| `PGPASSWORD`                       | `just create-database`, `just database` |
| `PGDATABASE`                       | `just create-database`, `just database` |
| `PGADMINUSER`                      | `just create-database`                  |
| `PGADMINPASSWORD`                  | `just create-database`                  |
| `BOOKBOSS__DATABASE__DATABASE_URL` | Migrations, entity generation           |

Connection string format for `BOOKBOSS__DATABASE__DATABASE_URL`:

- PostgreSQL: `postgres://user:password@host:port/database`
- MySQL: `mysql://user:password@host:port/database`
- SQLite: `sqlite::/path/to/file` or `sqlite::memory:`

> **Warning:** Secrets must be encrypted with `sops`. Never commit plaintext credentials.

## Migrations

```bash
just migrations    # redo all migrations
```

## Entity Generation

After schema changes, regenerate SeaORM entities:

```bash
just entities
```

## Integration Tests

Integration tests for each backend run in Docker via Colima:

```bash
colima start
just postgres-integration-tests
just mysql-integration-tests
just sqlite-integration-tests
colima stop
```
