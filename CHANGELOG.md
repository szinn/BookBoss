# BookBoss - Take Control Of Your Digital Library

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [unreleased]

### Features

- _(core)_ Add user domain with service, repository port, and tests - ([ddf56d4](https://github.com/szinn/BookBoss/commit/ddf56d46789a6e23e146787c7afdbc4a3b1e5f8c))
- _(core, database)_ Add auth domain with session management and login validation - ([56053ee](https://github.com/szinn/BookBoss/commit/56053eea4368e5c89f778067c06ddcfa6653d370))
- _(core, frontend)_ Add auth guard, Capability::as_str, and permission improvements - ([90e4456](https://github.com/szinn/BookBoss/commit/90e4456f8df7a951f3a47caf6b78ecad3ef06731))
- _(frontend)_ Replace NavBar text buttons with SVG icons - ([2c5582b](https://github.com/szinn/BookBoss/commit/2c5582b8708e1c1417a7e0d2166305b409432d3f))
- _(frontend)_ Add landing page with login and admin registration - ([f2d6f9f](https://github.com/szinn/BookBoss/commit/f2d6f9f7774129846be57db2b59a8b990d979465))
- _(frontend)_ Wire BackendSessionPool to AuthService and UserService - ([acfc6dc](https://github.com/szinn/BookBoss/commit/acfc6dc8107177307ab711cc594dab85b36d2572))
- _(frontend)_ Add favicon support - ([adfb6c7](https://github.com/szinn/BookBoss/commit/adfb6c70c3570e62b081dbd7693fe14ec99d2c4c))
- _(grpc)_ Move GRPC server to the grpc feature flag - ([07a1985](https://github.com/szinn/BookBoss/commit/07a1985a0ff6bb5a83d394c9d263a2dddc11d13f))
- _(metadata)_ Add stub crate for metadata services - ([d0ae6d6](https://github.com/szinn/BookBoss/commit/d0ae6d697c4409b40c161b6a253375435ab2ca22))

### Refactor

- _(frontend)_ Cleaning up and refactor - ([3d9114b](https://github.com/szinn/BookBoss/commit/3d9114bf7ad400faf996b50afc24b6fe2cbde0f8))
- _(frontend)_ Simplify extension extraction - ([8615bc4](https://github.com/szinn/BookBoss/commit/8615bc49334fbaab614b96af3563ce212c782000))
- _(frontend)_ Move functionality to real server module - ([2ad8409](https://github.com/szinn/BookBoss/commit/2ad8409b653e7d3d73da482df709a7d890c79b10))
- _(token)_ Randomize the alphabet for obscurity - ([6b0bf04](https://github.com/szinn/BookBoss/commit/6b0bf04f83e29dd2662784e516803aa108cb0acf))

### Testing

- _(core, database)_ Add unit and component tests for User model and adapters - ([c0a1e48](https://github.com/szinn/BookBoss/commit/c0a1e4828978446aa64a840f16374610a6448548))

### Miscellaneous Tasks

- Include generated tailwindcss file, Docker build - ([3de7067](https://github.com/szinn/BookBoss/commit/3de70677dd2851e5e3c0b504d4b5e9bfca39d4e8))
- Upgrade to tailwindss 4.2.1 - ([df40740](https://github.com/szinn/BookBoss/commit/df407402fd65846f178ab54907987673de695402))
