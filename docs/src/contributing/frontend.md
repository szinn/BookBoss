# Frontend (Dioxus)

The frontend is built with [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) in fullstack mode — server-side rendering with client-side hydration, using axum as the server.

> **Warning:** Dioxus 0.7 is a major API break from earlier versions. `cx`, `Scope`, and `use_state` are gone.
> Only use 0.7 documentation and patterns.

## Key Patterns

### Server Functions

Use `#[get]` or `#[put]` to define server functions. The macro takes the endpoint path followed by
any axum extensions the function needs, declared as `name: axum::Extension<Type>`. These are
injected server-side and are not part of the function's parameter list.

```rust
#[get("/api/v1/check_auth", auth_session: axum::Extension<AuthSession>)]
async fn check_auth() -> Result<bool, ServerFnError> {
    Ok(auth_session.current_user.as_ref().map(|u| !u.username.is_empty()).unwrap_or(false))
}
```

Function parameters (the request arguments) are declared normally on the function:

```rust
#[put("/api/v1/login", core_services: axum::Extension<Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
async fn perform_login(username: String, password: String) -> Result<(), ServerFnError> {
    // username and password come from the caller
    // core_services and auth_session are injected by axum
}
```

Use `#[tracing::instrument]` to add tracing — always `skip` the injected extensions:

```rust
#[get("/api/v1/get_landing_state", core_services: axum::Extension<Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
#[tracing::instrument(level = "trace", skip(core_services, auth_session))]
async fn get_landing_state() -> Result<LandingState, ServerFnError> { ... }
```

The server-side imports (`AuthSession`, `CoreServices`, etc.) are gated behind the `server` feature:

```rust
#[cfg(feature = "server")]
use {crate::server::AuthSession, bb_core::CoreServices, std::sync::Arc};
```

### Auth / Session

- `AuthSession` is stored in request extensions by `AuthSessionLayer`
- Check `!user.username.is_empty()` to determine if the user is authenticated (anonymous users have empty usernames)
- `auth_session.login_user(user_id)` logs in a user

### Routing

Routes are defined as a `Routable` enum. `LandingPage` lives outside `AppLayout` (no NavBar):

```rust
#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    LandingPage {},         // no layout — appears before #[layout(...)]

    #[layout(AppLayout)]
    #[route("/library")]
    LibraryPage {},
}
```

Navigate programmatically after a server fn succeeds:

```rust
let navigator = use_navigator();
navigator.push(Route::LibraryPage {});
```

### Hydration

Use `use_server_future` (not `use_resource`) for data that must be available on first render:

```rust
let data = use_server_future(fetch_data)?;
```

Browser-specific code (e.g. `localStorage`) must go inside `use_effect`, which runs only after hydration.

## Frontend Structure

```
crates/frontend/src/
├── routes/
│   └── landing_page.rs       # LandingPage + server fns
└── components/
    ├── login_form.rs
    └── register_admin_form.rs
```
