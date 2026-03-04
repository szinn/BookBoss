#!/usr/bin/env -S just --justfile

set unstable := true
set quiet := true
set shell := ['bash', '-euo', 'pipefail', '-c']

[private]
default:
    just -l

[doc('Install tooling for contributing to this project')]
install-tools:
    mise install
    rustup toolchain add nightly
    rustup target add wasm32-unknown-unknown

[doc('Edit encrypted configuration')]
config:
    sops -d -i config.sops.env
    nvim config.sops.env
    sops -e -i config.sops.env

[doc('Format code and documentation')]
fmt:
    cargo +nightly fmt --all
    prettier --config .config/prettierrc --ignore-path .gitignore --ignore-path .config/prettierignore --log-level warn -w .

[doc('Update CHANGELOG.md')]
changelog:
    jj sync
    RUST_LOG= git-cliff --config .config/cliff.toml > CHANGELOG.md
    just fmt

[doc('Build all applications')]
build:
    just tailwindcss
    cargo build --bin bookboss --all-features

[doc('Run BookBoss')]
run:
    just tailwindcss
    dx serve --web --package bookboss --args server

[doc('Create a release')]
release VERSION:
    ./scripts/release.sh {{ VERSION }}

[doc('Run Clippy on codebase for linting')]
clippy:
    cargo +nightly clippy --workspace --all-targets --all-features

[doc('Update rust crate dependencies')]
deps:
    cargo upgrade

[doc('Update tailwindcss')]
tailwindcss:
    tailwindcss -i ./crates/frontend/assets/input.css -o ./crates/frontend/assets/tailwind.css

[doc('Run quick tests using nextest')]
quick-test:
    just component-tests
    just postgres-integration-tests
    just sqlite-integration-tests

[doc('Run all tests using nextest')]
test:
    just component-tests
    just integration-tests

[doc('Run all tests using insta')]
insta:
    cargo insta test --workspace --test-runner nextest --all-features

[doc('Run all component tests using nextest')]
component-tests:
    cargo nextest run --workspace --all-features --exclude integration-tests

[doc('Run all integration tests using nextest')]
integration-tests:
    just postgres-integration-tests
    just sqlite-integration-tests
    just mysql-integration-tests

[doc('Run Postgres integration tests')]
postgres-integration-tests:
    cargo nextest run --no-default-features --features postgres --package integration-tests

[doc('Run SQLite integration tests')]
sqlite-integration-tests:
    cargo nextest run --no-default-features --features sqlite --package integration-tests

[doc('Run MySQL integration tests')]
mysql-integration-tests:
    cargo nextest run --no-default-features --features mysql --package integration-tests

[doc('Serve documentation locally with live reload')]
docs-serve:
    cd docs && mdbook serve

[doc('Build documentation as static site')]
docs-build:
    cd docs && mdbook build

[doc('Clean project workspace')]
clean:
    cargo clean

[doc('Database Admin')]
database:
    PGUSER=$PGADMINUSER PGPASSWORD=$PGADMINPASSWORD PGDATABASE= psql-18

[doc('Create the postgres database')]
create-database:
    #!/usr/bin/env bash
    set -euo pipefail

    SQL="""
      DROP DATABASE IF EXISTS "${PGDATABASE}";
      DROP ROLE IF EXISTS "${PGUSER}";

      CREATE ROLE "${PGUSER}" WITH
        LOGIN
        NOSUPERUSER
        INHERIT
        NOCREATEDB
        NOCREATEROLE
        NOREPLICATION
        PASSWORD '$PGPASSWORD';

      CREATE DATABASE "${PGDATABASE}"
        WITH
        OWNER = "${PGUSER}"
        ENCODING = 'UTF8'
        LC_COLLATE = 'C'
        LC_CTYPE = 'C'
        TABLESPACE = pg_default
        CONNECTION LIMIT = -1
        IS_TEMPLATE = False;

      GRANT TEMPORARY, CONNECT ON DATABASE "${PGDATABASE}" TO PUBLIC;

      GRANT ALL ON DATABASE "${PGDATABASE}" TO "${PGUSER}";
    """
    echo $SQL | PGUSER=$PGADMINUSER PGPASSWORD=$PGADMINPASSWORD PGDATABASE= psql-18 postgres
