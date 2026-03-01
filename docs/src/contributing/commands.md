# Commands

All commands are run via `just`.

## Development

| Command            | Description                    |
| ------------------ | ------------------------------ |
| `just build`       | Build the project              |
| `just run`         | Run the application            |
| `just fmt`         | Format code (nightly rustfmt)  |
| `just clippy`      | Run clippy lints               |
| `just clean`       | Clean the workspace            |
| `just deps`        | Update Rust crate dependencies |
| `just tailwindcss` | Update Tailwind CSS            |
| `just config`      | Edit encrypted configuration   |

## Testing

| Command                           | Description                |
| --------------------------------- | -------------------------- |
| `just quick-test`                 | Component + Postgres tests |
| `just test`                       | All tests                  |
| `just component-tests`            | Component tests only       |
| `just integration-tests`          | All integration tests      |
| `just postgres-integration-tests` | Postgres integration tests |
| `just sqlite-integration-tests`   | SQLite integration tests   |
| `just mysql-integration-tests`    | MySQL integration tests    |
| `just insta`                      | Run insta snapshot tests   |

## Database

| Command                | Description         |
| ---------------------- | ------------------- |
| `just database`        | Database admin      |
| `just create-database` | Create the database |

## Release

| Command              | Description          |
| -------------------- | -------------------- |
| `just changelog`     | Generate changelog   |
| `just install-tools` | Install/update tools |
