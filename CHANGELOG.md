# BookBoss - Take Control Of Your Digital Library

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [unreleased]

### Features

- _(core)_ Add user domain with service, repository port, and tests - ([ddf56d4](https://github.com/szinn/BookBoss/commit/ddf56d46789a6e23e146787c7afdbc4a3b1e5f8c))
- _(core, database)_ Add auth domain with session management and login validation - ([56053ee](https://github.com/szinn/BookBoss/commit/56053eea4368e5c89f778067c06ddcfa6653d370))
- _(core, frontend)_ Add auth guard, Capability::as_str, and permission improvements - ([5cfc635](https://github.com/szinn/BookBoss/commit/5cfc6359d2eef2f30253dc8131b15f7a280a5d32))
- _(core,database)_ Add per-user key/value settings store - ([362e3ea](https://github.com/szinn/BookBoss/commit/362e3ead42e391a7c2dfbd30ce2227077fce703a))
- _(frontend)_ Add GridView and NavBar view toggle for library - ([1e67cfe](https://github.com/szinn/BookBoss/commit/1e67cfef8db2c457c15aec7b21d4f04e97aee870))
- _(frontend)_ Add BookBoss title image to NavBar - ([9af471e](https://github.com/szinn/BookBoss/commit/9af471e20774bbda748d43314f03eb9927f1c40b))
- _(frontend)_ Add typed user settings and AuthUser get/set API - ([9e0a6d6](https://github.com/szinn/BookBoss/commit/9e0a6d637bfa8cbebd94733a0a1a8c7051388c8d))
- _(frontend)_ Replace NavBar text buttons with SVG icons - ([b73aacc](https://github.com/szinn/BookBoss/commit/b73aacc216a04dbf9bcf11301896f3683bb78f62))
- _(frontend)_ Add landing page with login and admin registration - ([f2d6f9f](https://github.com/szinn/BookBoss/commit/f2d6f9f7774129846be57db2b59a8b990d979465))
- _(frontend)_ Wire BackendSessionPool to AuthService and UserService - ([acfc6dc](https://github.com/szinn/BookBoss/commit/acfc6dc8107177307ab711cc594dab85b36d2572))
- _(frontend)_ Add favicon support - ([adfb6c7](https://github.com/szinn/BookBoss/commit/adfb6c70c3570e62b081dbd7693fe14ec99d2c4c))
- _(grpc)_ Move GRPC server to the grpc feature flag - ([07a1985](https://github.com/szinn/BookBoss/commit/07a1985a0ff6bb5a83d394c9d263a2dddc11d13f))
- _(metadata)_ Add stub crate for metadata services - ([d0ae6d6](https://github.com/szinn/BookBoss/commit/d0ae6d697c4409b40c161b6a253375435ab2ca22))

### Refactor

- _(frontend)_ Cleaning up and refactor - ([5f44656](https://github.com/szinn/BookBoss/commit/5f44656712f1a9587d2108a2c711ff0c0d1b66ca))
- _(frontend)_ Simplify extension extraction - ([8615bc4](https://github.com/szinn/BookBoss/commit/8615bc49334fbaab614b96af3563ce212c782000))
- _(frontend)_ Move functionality to real server module - ([2ad8409](https://github.com/szinn/BookBoss/commit/2ad8409b653e7d3d73da482df709a7d890c79b10))
- _(token)_ Randomize the alphabet for obscurity - ([6b0bf04](https://github.com/szinn/BookBoss/commit/6b0bf04f83e29dd2662784e516803aa108cb0acf))

### Documentation

- Add mdbook documentation with user guide and contributor sections - ([9b8da28](https://github.com/szinn/BookBoss/commit/9b8da289c8270b8852a58754fc685058e70718ac))

### Testing

- _(core, database)_ Add unit and component tests for User model and adapters - ([c0a1e48](https://github.com/szinn/BookBoss/commit/c0a1e4828978446aa64a840f16374610a6448548))
