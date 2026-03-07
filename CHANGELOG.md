# BookBoss - Take Control Of Your Digital Library

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.13](https://github.com/szinn/BookBoss/compare/v0.1.12..v0.1.13) - 2026-03-07

### Features

- _(bookboss)_ Wire scanner and worker subsystems into server startup - ([c919c74](https://github.com/szinn/BookBoss/commit/c919c741338360fab67117d8aa8cd0d369cb3516))
- _(core)_ Include first author in stored book file slug - ([9ef8b0b](https://github.com/szinn/BookBoss/commit/9ef8b0b39b21280a6ad08e6d83e3d2562ea08f2c))
- _(database)_ Case-insensitive find_by_name across entity adapters - ([b7d22f8](https://github.com/szinn/BookBoss/commit/b7d22f8ae60fae0e73e2dfc7905eadbd203147c1))
- _(database)_ Add JobRepositoryAdapter with optimistic-locking claim loop - ([d6ec64a](https://github.com/szinn/BookBoss/commit/d6ec64a419f73cee44cd1f47a2b6dccc9cb0d92e))
- _(formats,core)_ Extract cover image from EPUB file - ([3cbf48c](https://github.com/szinn/BookBoss/commit/3cbf48cdd9141eda19dd503f678d9f684d278993))
- _(import)_ Add bb-import crate with scanner subsystem and process_import handler - ([8b0f9b4](https://github.com/szinn/BookBoss/commit/8b0f9b454bc2fa98dd8683696ec3adb1e5aa8e08))

### Bug Fixes

- _(api)_ Enable TLS for gRPC client connections - ([75a660c](https://github.com/szinn/BookBoss/commit/75a660c85189a0b84960243475b8617e72b4f440))
- _(import,core)_ Pipeline robustness and startup recovery - ([30d4825](https://github.com/szinn/BookBoss/commit/30d48251d2c1080b36346d64f645e2db0cb5e8c5))

### Documentation

- Add storage crate and M3 config vars to README and docs - ([84e56ea](https://github.com/szinn/BookBoss/commit/84e56ea158d1a29a78061df9c6a64d955a82fe19))

## [0.1.12](https://github.com/szinn/BookBoss/compare/v0.1.11..v0.1.12) - 2026-03-06

### Features

- _(bookboss)_ Add ImportConfig with watch_directory and poll_interval_secs - ([fe22822](https://github.com/szinn/BookBoss/commit/fe228229e2a9e2a34630189d5867f3d3096e238b))
- _(core)_ Add job handler registry and worker subsystem - ([060c6b3](https://github.com/szinn/BookBoss/commit/060c6b3db06bd8b9112313f4b817214f378d1753))
- _(core,database)_ Add job queue port traits and wire into RepositoryService - ([7d11484](https://github.com/szinn/BookBoss/commit/7d11484ada1f8559facc1189c8bedd36215d03b9))
- _(database)_ Add jobs table migration and entity for job queue - ([d538345](https://github.com/szinn/BookBoss/commit/d538345a9ad104b193a491636749fd072eab035d))
- _(grpc)_ Add reflection API - ([c982937](https://github.com/szinn/BookBoss/commit/c982937efdcbe985d39ead0f7b45bce9a3d0e1b6))

## [0.1.11](https://github.com/szinn/BookBoss/compare/v0.1.10..v0.1.11) - 2026-03-05

### Features

- _(api,cli)_ Add grpc CLI command and fix system status endpoint - ([38d13c2](https://github.com/szinn/BookBoss/commit/38d13c2d54c533937cb442e101ea50539622cf42))
- _(cli)_ Add dump-epub command for exploring EPUB metadata - ([7c42900](https://github.com/szinn/BookBoss/commit/7c42900ae069357c3a9a0a11f70cd6adf267f0c5))
- _(core)_ Add PipelineService for M3.11 acquisition pipeline - ([85dae12](https://github.com/szinn/BookBoss/commit/85dae125f5445a4d77aa4fb5682d37f6e8bf15f1))
- _(formats)_ Add read_opf_metadata_xml helper for diagnostics - ([4f24509](https://github.com/szinn/BookBoss/commit/4f2450996f83b2614a9e669f61f405528a305ed6))
- _(formats)_ EPUB metadata extraction - ([af39d9f](https://github.com/szinn/BookBoss/commit/af39d9fb99f38714a2a673fe3d4f7b553557d4b4))
- _(formats)_ OPF sidecar parse and write - ([0e08af3](https://github.com/szinn/BookBoss/commit/0e08af3e329595d7e5b476238b46581f7f223f31))
- _(metadata)_ Add openlibrary CLI command and Default impl - ([c02f304](https://github.com/szinn/BookBoss/commit/c02f304e5811ea0a30dbe7606ae091a2224282c6))
- _(metadata,cli)_ Add HardcoverAdapter MetadataProvider and CLI command - ([edd6b5b](https://github.com/szinn/BookBoss/commit/edd6b5b42cc1bb5336dc043d695f259da629ec00))
- _(storage)_ Implement LocalLibraryStore for local filesystem storage - ([d7c7f7e](https://github.com/szinn/BookBoss/commit/d7c7f7e2717dd06dca49f5373131440028b9f574))
- _(utils)_ Add hash_file utility for SHA-256 file hashing - ([73a3aff](https://github.com/szinn/BookBoss/commit/73a3affe5d0ac07c016ee57c1e9964ee24561849))

### Bug Fixes

- _(formats)_ Migrate quick-xml unescape to decode for v0.39 - ([a19a8f3](https://github.com/szinn/BookBoss/commit/a19a8f3d82a085784e14b952e5e27a3fd63a6973))

### Refactor

- _(metadata,core)_ Provider chain with name() and create_metadata_providers() - ([d816f68](https://github.com/szinn/BookBoss/commit/d816f68b99c3d0c91b9de62a78715ad4c85bcfee))

### Testing

- _(formats)_ Add insta regression test suite for OPF parsing - ([5166ed2](https://github.com/szinn/BookBoss/commit/5166ed286a8391f83f8e6c67e7d5288d411551ca))

## [0.1.10](https://github.com/szinn/BookBoss/compare/v0.1.8..v0.1.10) - 2026-03-03

### Features

- _(core)_ Add ImportJobService with approve/reject transitions (M3.5) - ([fb894da](https://github.com/szinn/BookBoss/commit/fb894da2ce7ee25cfec5402abcad8b0a2811b388))
- _(core)_ Add pipeline port traits and models (M3.3) - ([8baa337](https://github.com/szinn/BookBoss/commit/8baa3379ebe48afe63c39060003785cc885575e5))
- _(core)_ Add LibraryStore port trait and BookSidecar struct (M3.2) - ([d6876ec](https://github.com/szinn/BookBoss/commit/d6876ec0d765880be614646afdf611851150e082))
- _(core,database)_ Add ImportJobRepository port and adapter (M3.4) - ([89afb54](https://github.com/szinn/BookBoss/commit/89afb5437508b6903994ef35db74c5d98507973d))
- _(database,core)_ Drop file_path from book_files (M3.1) - ([e785ee5](https://github.com/szinn/BookBoss/commit/e785ee57c22d94796c21eeab39d4d50aca33db67))
- _(frontend)_ Add series detail page (M2.7) - ([0bdfaec](https://github.com/szinn/BookBoss/commit/0bdfaec7da13d990eaebc2e3fb7805ebc5730657))
- _(frontend)_ Add author detail page (M2.6) - ([5cc4de8](https://github.com/szinn/BookBoss/commit/5cc4de8391f24c0cbc6d80632cd42e5d8239a99c))
- _(frontend)_ Add book detail page at /library/books/:token (M2.5) - ([9b1e670](https://github.com/szinn/BookBoss/commit/9b1e67083a091305514567cfad3d688d2f49428e))

### Bug Fixes

- _(frontend)_ Use #[post] instead of #[put] for get_book server fn - ([860943a](https://github.com/szinn/BookBoss/commit/860943a0a2eac9896106cb687b3912efe2d4b200))
- _(frontend)_ Require authentication on GET /api/v1/books - ([351c076](https://github.com/szinn/BookBoss/commit/351c0762f914abe4883355ecd335dd1bfe2128b3))

### Miscellaneous Tasks

- _(Dockerfile)_ Don't worry about target labels yet - ([613c637](https://github.com/szinn/BookBoss/commit/613c6372f0c5f9945ad58ba9c1be2143c28e0ac4))
- _(database)_ Squash drop migration - ([244acc1](https://github.com/szinn/BookBoss/commit/244acc13ccb83d3ee4ff4edd22dcc9bceba4749b))

## [0.1.8](https://github.com/szinn/BookBoss/compare/v0.1.7..v0.1.8) - 2026-03-03

### Features

- _(core)_ Implement BookService + wire into CoreServices (M2.3) - ([8e1e158](https://github.com/szinn/BookBoss/commit/8e1e158f010ba4235894016ddad5313e7d6d5cc0))
- _(core)_ Add import domain models and port trait (M1.8) - ([f85b71a](https://github.com/szinn/BookBoss/commit/f85b71a19a1e470005277339f6f9973e9862a6b5))
- _(core)_ Add shelf domain models and port trait (M1.7) - ([0544c9c](https://github.com/szinn/BookBoss/commit/0544c9cdf9fa31200ef2a4f0fb0b2a79e6e7fd9b))
- _(core,database)_ Implement BookRepositoryAdapter (M2.2) - ([fb0a169](https://github.com/szinn/BookBoss/commit/fb0a16950c84c8c491d69fadc47621649d34bc1a))
- _(core,database)_ Implement M2.1 reference table adapters - ([4fc67d5](https://github.com/szinn/BookBoss/commit/4fc67d56bb5be3708c83d20ec14f2dd15c41dc2c))
- _(core,database)_ Add user state and device table migrations and entities - ([f7f64ba](https://github.com/szinn/BookBoss/commit/f7f64baa55c3dbb1b4dfab26c16109244a9970dd))
- _(core,database)_ Add core book table migrations and entities - ([dc0e38a](https://github.com/szinn/BookBoss/commit/dc0e38a3f743035cb717650607630253fe063750))
- _(core,database)_ Add catalog reference table migrations and entities - ([e1a260e](https://github.com/szinn/BookBoss/commit/e1a260ea387f9b0a06576ddef69f869d314ec8f5))
- _(core,frontend)_ Add SuperAdmin capability and display app version - ([d4f6a3a](https://github.com/szinn/BookBoss/commit/d4f6a3ac018eafcdecc9074c6f9107d324948afb))
- _(frontend)_ Wire library page to real book data (M2.4) - ([e0bea77](https://github.com/szinn/BookBoss/commit/e0bea77889dde988a22ba70aa2a2a3c9447be3c8))

### Documentation

- Add project README - ([6a6d984](https://github.com/szinn/BookBoss/commit/6a6d9841d2f055ec934237b49b465ef666535745))

### Testing

- _(core)_ Add serde round-trip tests for ReadStatus and ShelfFilter - ([83fedec](https://github.com/szinn/BookBoss/commit/83fedecdb39fdd51f0afd631ffba2dc258335abb))

## [0.1.7] - 2026-03-01

### Features

- _(core)_ Add user domain with service, repository port, and tests - ([ddf56d4](https://github.com/szinn/BookBoss/commit/ddf56d46789a6e23e146787c7afdbc4a3b1e5f8c))
- _(core, database)_ Add auth domain with session management and login validation - ([56053ee](https://github.com/szinn/BookBoss/commit/56053eea4368e5c89f778067c06ddcfa6653d370))
- _(core, frontend)_ Add auth guard, Capability::as_str, and permission improvements - ([5cfc635](https://github.com/szinn/BookBoss/commit/5cfc6359d2eef2f30253dc8131b15f7a280a5d32))
- _(core,database)_ Add per-user key/value settings store - ([362e3ea](https://github.com/szinn/BookBoss/commit/362e3ead42e391a7c2dfbd30ce2227077fce703a))
- _(frontend)_ Add settings page with About section - ([bb4ba61](https://github.com/szinn/BookBoss/commit/bb4ba61a13d2cbac8f1773ba6a4777ed8dd713f1))
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

- Add mdbook documentation with user guide and contributor sections - ([41de843](https://github.com/szinn/BookBoss/commit/41de8435fa6b56c1feb80dd888f18be8bcb3e5f2))

### Testing

- _(core, database)_ Add unit and component tests for User model and adapters - ([c0a1e48](https://github.com/szinn/BookBoss/commit/c0a1e4828978446aa64a840f16374610a6448548))

### Miscellaneous Tasks

- _(release)_ Add release script and GitHub Actions workflows - ([962f3ec](https://github.com/szinn/BookBoss/commit/962f3ec80bec55e86a83b0ff870d44aaf7d8a421))
