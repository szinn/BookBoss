# Conventions

## Version Control

This project uses [jujutsu](https://github.com/jj-vcs/jj) (`jj`), not `git` directly.

Key commands:

| Command       | Description                         |
| ------------- | ----------------------------------- |
| `jj commit`   | Commit current changes              |
| `jj describe` | Update the working copy description |
| `jj new`      | Start a new change                  |
| `jj log`      | Show history                        |
| `jj status`   | Show working copy status            |

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/) with crate-based scopes, sorted alphabetically:

```
type(scope): description
```

Valid scopes: `api`, `cli`, `core`, `database`, `frontend` (match crate names).

Examples:

```
feat(core): add book domain with service and repository port
fix(database): handle null author field in entity mapping
refactor(frontend): simplify extension extraction
```

Use `jj describe -m "..."` to set the working copy description. Do not amend published commits.

## Error Handling

| Crate                           | Approach                     |
| ------------------------------- | ---------------------------- |
| `core`, `api`, `database`       | `thiserror` for typed errors |
| `bookboss` (binary entry point) | `anyhow` for ad-hoc errors   |

## Dependencies

All crate dependencies are defined in the root `Cargo.toml` under `[workspace.dependencies]`.
Individual crates reference them with `crate-name.workspace = true`.

In root `Cargo.toml`:

- Version-only deps: inline format — `anyhow = "1.0.100"`
- Deps with features: section format:

```toml
[workspace.dependencies.uuid]
version = "1"
features = ["v4", "serde"]
```

## Secrets

Secrets are encrypted with `sops`. Never commit plaintext secrets.

## Testing

- Use `cargo-nextest` as the test runner (`just test`)
- Use `cargo-insta` for snapshot tests when asserting against larger or structured output
- Use regular assertions for simple value checks
- Tests live alongside source code in `#[cfg(test)]` modules

## After Completing Work

1. Run component tests: `just component-tests`
2. Run all tests: `just test`
3. Run clippy: `just clippy`
4. Format: `just fmt`
5. Update the working copy description: `jj desc -m "type(scope): description"`
