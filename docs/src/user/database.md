# Database Configuration

BookBoss supports three database backends. Choose the one that fits your deployment:

| Database   | Best for                                    |
| ---------- | ------------------------------------------- |
| SQLite     | Single-user, low maintenance, simple setups |
| PostgreSQL | Multi-user, production deployments          |
| MySQL      | Existing MySQL infrastructure               |

---

## SQLite

SQLite requires no separate server — the database is a single file on disk.

Set the `database_url` in your configuration to a file path:

```toml
[database]
database_url = "sqlite:///path/to/bookboss.db"
```

Or use a relative path:

```toml
[database]
database_url = "sqlite://./bookboss.db"
```

> **Tip:** SQLite is the simplest option for personal use. No additional software required.

---

## PostgreSQL

PostgreSQL is recommended for multi-user or production deployments.

### Prerequisites

A running PostgreSQL instance is required. You can run one with Docker:

```bash
docker run -d \
  --name bookboss-postgres \
  -e POSTGRES_USER=bookboss \
  -e POSTGRES_PASSWORD=yourpassword \
  -e POSTGRES_DB=bookboss \
  -p 5432:5432 \
  postgres:16
```

### Configuration

```toml
[database]
database_url = "postgres://user:password@host:5432/database"
```

For example:

```toml
[database]
database_url = "postgres://bookboss:yourpassword@localhost:5432/bookboss"
```

---

## MySQL

### Prerequisites

A running MySQL instance is required. You can run one with Docker:

```bash
docker run -d \
  --name bookboss-mysql \
  -e MYSQL_USER=bookboss \
  -e MYSQL_PASSWORD=yourpassword \
  -e MYSQL_DATABASE=bookboss \
  -e MYSQL_ROOT_PASSWORD=rootpassword \
  -p 3306:3306 \
  mysql:8
```

### Configuration

```toml
[database]
database_url = "mysql://user:password@host:3306/database"
```

For example:

```toml
[database]
database_url = "mysql://bookboss:yourpassword@localhost:3306/bookboss"
```

---

## Migrations

BookBoss applies database migrations automatically on startup. No manual steps are required.
