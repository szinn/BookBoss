# BookBoss - Take Control Of Your Digital Library

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.1.10](https://github.com/szinn/BookBoss/compare/v0.1.8..v0.1.10) - 2026-03-03


### Features
- *(core)* Add ImportJobService with approve/reject transitions (M3.5) - ([fb894da](https://github.com/szinn/BookBoss/commit/fb894da2ce7ee25cfec5402abcad8b0a2811b388))
- *(core)* Add pipeline port traits and models (M3.3) - ([8baa337](https://github.com/szinn/BookBoss/commit/8baa3379ebe48afe63c39060003785cc885575e5))
- *(core)* Add LibraryStore port trait and BookSidecar struct (M3.2) - ([d6876ec](https://github.com/szinn/BookBoss/commit/d6876ec0d765880be614646afdf611851150e082))
- *(core,database)* Add ImportJobRepository port and adapter (M3.4) - ([89afb54](https://github.com/szinn/BookBoss/commit/89afb5437508b6903994ef35db74c5d98507973d))
- *(database,core)* Drop file_path from book_files (M3.1) - ([e785ee5](https://github.com/szinn/BookBoss/commit/e785ee57c22d94796c21eeab39d4d50aca33db67))
- *(deps)* Update crate tokio to 1.50.0 ([#14](https://github.com/szinn/BookBoss/issues/14)) - ([51f6d79](https://github.com/szinn/BookBoss/commit/51f6d79af9dc72a3a05f6d124a8bfb754cf6add6))
- *(frontend)* Add series detail page (M2.7) - ([0bdfaec](https://github.com/szinn/BookBoss/commit/0bdfaec7da13d990eaebc2e3fb7805ebc5730657))
- *(frontend)* Add author detail page (M2.6) - ([5cc4de8](https://github.com/szinn/BookBoss/commit/5cc4de8391f24c0cbc6d80632cd42e5d8239a99c))
- *(frontend)* Add book detail page at /library/books/:token (M2.5) - ([9b1e670](https://github.com/szinn/BookBoss/commit/9b1e67083a091305514567cfad3d688d2f49428e))

### Bug Fixes
- *(frontend)* Use #[post] instead of #[put] for get_book server fn - ([860943a](https://github.com/szinn/BookBoss/commit/860943a0a2eac9896106cb687b3912efe2d4b200))
- *(frontend)* Require authentication on GET /api/v1/books - ([351c076](https://github.com/szinn/BookBoss/commit/351c0762f914abe4883355ecd335dd1bfe2128b3))

### Miscellaneous Tasks
- *(Dockerfile)* Don't worry about target labels yet - ([613c637](https://github.com/szinn/BookBoss/commit/613c6372f0c5f9945ad58ba9c1be2143c28e0ac4))
- *(database)* Squash drop migration - ([244acc1](https://github.com/szinn/BookBoss/commit/244acc13ccb83d3ee4ff4edd22dcc9bceba4749b))

## [0.1.8](https://github.com/szinn/BookBoss/compare/v0.1.7..v0.1.8) - 2026-03-03


### Features
- *(core)* Implement BookService + wire into CoreServices (M2.3) - ([8e1e158](https://github.com/szinn/BookBoss/commit/8e1e158f010ba4235894016ddad5313e7d6d5cc0))
- *(core)* Add import domain models and port trait (M1.8) - ([f85b71a](https://github.com/szinn/BookBoss/commit/f85b71a19a1e470005277339f6f9973e9862a6b5))
- *(core)* Add shelf domain models and port trait (M1.7) - ([0544c9c](https://github.com/szinn/BookBoss/commit/0544c9cdf9fa31200ef2a4f0fb0b2a79e6e7fd9b))
- *(core,database)* Implement BookRepositoryAdapter (M2.2) - ([fb0a169](https://github.com/szinn/BookBoss/commit/fb0a16950c84c8c491d69fadc47621649d34bc1a))
- *(core,database)* Implement M2.1 reference table adapters - ([4fc67d5](https://github.com/szinn/BookBoss/commit/4fc67d56bb5be3708c83d20ec14f2dd15c41dc2c))
- *(core,database)* Add user state and device table migrations and entities - ([f7f64ba](https://github.com/szinn/BookBoss/commit/f7f64baa55c3dbb1b4dfab26c16109244a9970dd))
- *(core,database)* Add core book table migrations and entities - ([dc0e38a](https://github.com/szinn/BookBoss/commit/dc0e38a3f743035cb717650607630253fe063750))
- *(core,database)* Add catalog reference table migrations and entities - ([e1a260e](https://github.com/szinn/BookBoss/commit/e1a260ea387f9b0a06576ddef69f869d314ec8f5))
- *(core,frontend)* Add SuperAdmin capability and display app version - ([d4f6a3a](https://github.com/szinn/BookBoss/commit/d4f6a3ac018eafcdecc9074c6f9107d324948afb))
- *(frontend)* Wire library page to real book data (M2.4) - ([e0bea77](https://github.com/szinn/BookBoss/commit/e0bea77889dde988a22ba70aa2a2a3c9447be3c8))

### Documentation
- Add project README - ([6a6d984](https://github.com/szinn/BookBoss/commit/6a6d9841d2f055ec934237b49b465ef666535745))

### Testing
- *(core)* Add serde round-trip tests for ReadStatus and ShelfFilter - ([83fedec](https://github.com/szinn/BookBoss/commit/83fedecdb39fdd51f0afd631ffba2dc258335abb))

## [0.1.7] - 2026-03-01


### Features
- *(core)* Add user domain with service, repository port, and tests - ([ddf56d4](https://github.com/szinn/BookBoss/commit/ddf56d46789a6e23e146787c7afdbc4a3b1e5f8c))
- *(core, database)* Add auth domain with session management and login validation - ([56053ee](https://github.com/szinn/BookBoss/commit/56053eea4368e5c89f778067c06ddcfa6653d370))
- *(core, frontend)* Add auth guard, Capability::as_str, and permission improvements - ([5cfc635](https://github.com/szinn/BookBoss/commit/5cfc6359d2eef2f30253dc8131b15f7a280a5d32))
- *(core,database)* Add per-user key/value settings store - ([362e3ea](https://github.com/szinn/BookBoss/commit/362e3ead42e391a7c2dfbd30ce2227077fce703a))
- *(frontend)* Add settings page with About section - ([bb4ba61](https://github.com/szinn/BookBoss/commit/bb4ba61a13d2cbac8f1773ba6a4777ed8dd713f1))
- *(frontend)* Add GridView and NavBar view toggle for library - ([1e67cfe](https://github.com/szinn/BookBoss/commit/1e67cfef8db2c457c15aec7b21d4f04e97aee870))
- *(frontend)* Add BookBoss title image to NavBar - ([9af471e](https://github.com/szinn/BookBoss/commit/9af471e20774bbda748d43314f03eb9927f1c40b))
- *(frontend)* Add typed user settings and AuthUser get/set API - ([9e0a6d6](https://github.com/szinn/BookBoss/commit/9e0a6d637bfa8cbebd94733a0a1a8c7051388c8d))
- *(frontend)* Replace NavBar text buttons with SVG icons - ([b73aacc](https://github.com/szinn/BookBoss/commit/b73aacc216a04dbf9bcf11301896f3683bb78f62))
- *(frontend)* Add landing page with login and admin registration - ([f2d6f9f](https://github.com/szinn/BookBoss/commit/f2d6f9f7774129846be57db2b59a8b990d979465))
- *(frontend)* Wire BackendSessionPool to AuthService and UserService - ([acfc6dc](https://github.com/szinn/BookBoss/commit/acfc6dc8107177307ab711cc594dab85b36d2572))
- *(frontend)* Add favicon support - ([adfb6c7](https://github.com/szinn/BookBoss/commit/adfb6c70c3570e62b081dbd7693fe14ec99d2c4c))
- *(grpc)* Move GRPC server to the grpc feature flag - ([07a1985](https://github.com/szinn/BookBoss/commit/07a1985a0ff6bb5a83d394c9d263a2dddc11d13f))
- *(metadata)* Add stub crate for metadata services - ([d0ae6d6](https://github.com/szinn/BookBoss/commit/d0ae6d697c4409b40c161b6a253375435ab2ca22))

### Refactor
- *(frontend)* Cleaning up and refactor - ([5f44656](https://github.com/szinn/BookBoss/commit/5f44656712f1a9587d2108a2c711ff0c0d1b66ca))
- *(frontend)* Simplify extension extraction - ([8615bc4](https://github.com/szinn/BookBoss/commit/8615bc49334fbaab614b96af3563ce212c782000))
- *(frontend)* Move functionality to real server module - ([2ad8409](https://github.com/szinn/BookBoss/commit/2ad8409b653e7d3d73da482df709a7d890c79b10))
- *(token)* Randomize the alphabet for obscurity - ([6b0bf04](https://github.com/szinn/BookBoss/commit/6b0bf04f83e29dd2662784e516803aa108cb0acf))

### Documentation
- Add mdbook documentation with user guide and contributor sections - ([41de843](https://github.com/szinn/BookBoss/commit/41de8435fa6b56c1feb80dd888f18be8bcb3e5f2))

### Testing
- *(core, database)* Add unit and component tests for User model and adapters - ([c0a1e48](https://github.com/szinn/BookBoss/commit/c0a1e4828978446aa64a840f16374610a6448548))

### Miscellaneous Tasks
- *(release)* Add release script and GitHub Actions workflows - ([962f3ec](https://github.com/szinn/BookBoss/commit/962f3ec80bec55e86a83b0ff870d44aaf7d8a421))
