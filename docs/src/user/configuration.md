# Configuration Reference

> **Note:** This reference will be completed as the configuration surface stabilises.

Configuration is loaded from environment variables with the prefix `BOOKBOSS` and `__` as the
separator (e.g. `BOOKBOSS__DATABASE__DATABASE_URL`).

## Database

| Variable                           | Description                               | Default |
| ---------------------------------- | ----------------------------------------- | ------- |
| `BOOKBOSS__DATABASE__DATABASE_URL` | Database connection string (**required**) | —       |

See [Database Configuration](database.md) for connection string format and examples.

## Frontend

| Variable                          | Description                          | Default   |
| --------------------------------- | ------------------------------------ | --------- |
| `BOOKBOSS__FRONTEND__LISTEN_IP`   | IP address the web server listens on | `0.0.0.0` |
| `BOOKBOSS__FRONTEND__LISTEN_PORT` | Port the web server listens on       | `8080`    |

## Library

| Variable                          | Description                                     | Default |
| --------------------------------- | ----------------------------------------------- | ------- |
| `BOOKBOSS__LIBRARY__LIBRARY_PATH` | Path where book files are stored (**required**) | —       |

## Import

| Variable                               | Description                                            | Default |
| -------------------------------------- | ------------------------------------------------------ | ------- |
| `BOOKBOSS__IMPORT__WATCH_DIRECTORY`    | Directory to watch for new e-book files (**required**) | —       |
| `BOOKBOSS__IMPORT__POLL_INTERVAL_SECS` | How often (seconds) to scan the watch directory        | `60`    |

## API (gRPC)

| Variable                          | Description                           | Default   |
| --------------------------------- | ------------------------------------- | --------- |
| `BOOKBOSS__API__GRPC_LISTEN_IP`   | IP address the gRPC server listens on | `0.0.0.0` |
| `BOOKBOSS__API__GRPC_LISTEN_PORT` | Port the gRPC server listens on       | `8081`    |
