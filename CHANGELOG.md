# BookBoss - Take Control Of Your Digital Library

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.34](https://github.com/szinn/BookBoss/compare/v0.8.33..v0.8.34) - 2026-08-23

### Features

- _(core)_ Define TokenAlphabet matching bb-utils's retired token alphabet - ([11bb4a2](https://github.com/szinn/BookBoss/commit/11bb4a27ddb6c1869857dc51d8fd3d448116300e))

### Bug Fixes

- _(formats)_ Migrate crates/formats to quick-xml 0.42 API - ([855447b](https://github.com/szinn/BookBoss/commit/855447bb5b3f96350d2e17429ee9bc36f67de97d))

### Refactor

- _(core)_ Migrate Token declarations to alderkit-token via TokenAlphabet - ([6663fcd](https://github.com/szinn/BookBoss/commit/6663fcd9320a324d4305b60c67433fba198899a2))

### Miscellaneous Tasks

- _(core)_ Prune bb-utils deps and MSRV after alderkit-token migration - ([2ee665f](https://github.com/szinn/BookBoss/commit/2ee665fefbd5e86b80f820e8b392f554f6ad1f7a))
- _(core)_ Retire bb-utils token implementation - ([2bf8a11](https://github.com/szinn/BookBoss/commit/2bf8a1140ac24cb24544ec351bdc0c2cf8d5b5e9))
- _(core)_ Add alderkit-token dependency - ([fcddac0](https://github.com/szinn/BookBoss/commit/fcddac0c0616ffdf910e630cf6891ac79b8e1f1a))
- Fix clippy warnings across all crates - ([66a3d45](https://github.com/szinn/BookBoss/commit/66a3d45c17da56d2063901de2f0f5438c19a073b))

## [0.8.33](https://github.com/szinn/BookBoss/compare/v0.8.32..v0.8.33) - 2026-08-02

### Bug Fixes

- _(database)_ Treat SQLite busy/locked errors as transient - ([1ce0129](https://github.com/szinn/BookBoss/commit/1ce01297e64783b1e6ccbb879bde555c83f1344b))

## [0.8.32](https://github.com/szinn/BookBoss/compare/v0.8.31..v0.8.32) - 2026-08-02

### Bug Fixes

- _(core)_ Update opds encryption for aes-gcm 0.11 API - ([3a46da8](https://github.com/szinn/BookBoss/commit/3a46da800051527c0c37a111c6ceeaa92cce4400))

## [0.8.30](https://github.com/szinn/BookBoss/compare/v0.8.29..v0.8.30) - 2026-05-25

### Bug Fixes

- _(frontend)_ Tolerate trailing-slash mismatch in OIDC issuer - ([33b82db](https://github.com/szinn/BookBoss/commit/33b82db311d1a52a418a01a8d37ef4d2aec39e46))

## [0.8.29](https://github.com/szinn/BookBoss/compare/v0.8.28..v0.8.29) - 2026-05-23

### Bug Fixes

- _(core)_ Dedupe genres and tags by resolved id during book writes - ([5ddfd84](https://github.com/szinn/BookBoss/commit/5ddfd84fa597b0eb1f76b55ee885a6f6169ffb3c))

### Miscellaneous Tasks

- _(formats)_ Migrate off deprecated quick_xml::Attribute::decode_and_unescape_value - ([167c2d2](https://github.com/szinn/BookBoss/commit/167c2d25d3533abc58cb599be7482b17d814ff3e))

## [0.8.28](https://github.com/szinn/BookBoss/compare/v0.8.27..v0.8.28) - 2026-05-14

### Refactor

- _(database)_ Make adapter pagination caller-owned - ([9cc6bb2](https://github.com/szinn/BookBoss/commit/9cc6bb2d72dbd9a9d8114bf58b940cf38bb4a4d6))

### Testing

- _(core)_ BB-18 reproducer for compute_sync_diff - ([2272d73](https://github.com/szinn/BookBoss/commit/2272d73b7a8b1144fc1b155f72719a61463acf80))
- _(database)_ Add unbounded pagination regressions - ([3657b70](https://github.com/szinn/BookBoss/commit/3657b70e2a20df68c12175a1a5d186f09a6635ec))

## [0.8.27](https://github.com/szinn/BookBoss/compare/v0.8.26..v0.8.27) - 2026-05-05

### Bug Fixes

- _(core)_ Dedupe authors in pipeline before linking to book - ([7948cfe](https://github.com/szinn/BookBoss/commit/7948cfe87acc8146a7aef84e46284ae7393174ac))

### Miscellaneous Tasks

- _(lint)_ Clear pedantic clippy warnings - ([0a51b00](https://github.com/szinn/BookBoss/commit/0a51b005e24e4c5338a82643ba76169703b390cc))

## [0.8.26](https://github.com/szinn/BookBoss/compare/v0.8.25..v0.8.26) - 2026-05-04

### Bug Fixes

- _(core)_ Scope manual shelf count_for_filter to membership only - ([c3ffd09](https://github.com/szinn/BookBoss/commit/c3ffd094ee788a497b1b874c1bf72b18aab0c575))
- _(formats)_ Accept non-self-closing rootfile in container.xml - ([2093af3](https://github.com/szinn/BookBoss/commit/2093af3f331b51484d6f5f87f44b6409536319cd))
- _(test)_ Insert books into ALL_BOOKS in integration fixture - ([0026438](https://github.com/szinn/BookBoss/commit/00264389c9fd98db26d08e0d5e87e4045b70bbb1))

### Testing

- _(integration)_ Add insert_book_into_library helper - ([06c1915](https://github.com/szinn/BookBoss/commit/06c19157b7263b55d5114047ec93055953de7688))
- _(integration)_ Add insert_library fixture helper - ([7dae035](https://github.com/szinn/BookBoss/commit/7dae0354c388595952c960b9c81d5a8b89a9b392))

## [0.8.25](https://github.com/szinn/BookBoss/compare/v0.8.24..v0.8.25) - 2026-05-04

### Bug Fixes

- _(database)_ Apply SQLite PRAGMAs to fix database is locked errors - ([2596d7d](https://github.com/szinn/BookBoss/commit/2596d7de9f87ac190c00ea98ff566d89e798f4d5))

### Documentation

- _(database)_ Note WAL sidecar files and SQLite PRAGMAs - ([a7d8aff](https://github.com/szinn/BookBoss/commit/a7d8affb7e3bd4e82cd745d15e92949ba6e252a1))

## [0.8.24](https://github.com/szinn/BookBoss/compare/v0.8.23..v0.8.24) - 2026-05-03

### Documentation

- _(user)_ Document MOBI conversion and admin settings - ([79473f5](https://github.com/szinn/BookBoss/commit/79473f5227e4613adcab6615b3ba420896a133bf))
- _(user)_ Document drag-and-drop upload and dark mode - ([33e82ee](https://github.com/szinn/BookBoss/commit/33e82eef5472f1d63b166258151d4f73ed8aa750))
- _(user)_ Add OIDC SSO documentation - ([c68eedd](https://github.com/szinn/BookBoss/commit/c68eedd8a12d316c6abd31aca4bb4882f1e9e8e4))

## [0.8.23](https://github.com/szinn/BookBoss/compare/v0.8.22..v0.8.23) - 2026-05-01

### Bug Fixes

- _(frontend)_ Surface full error chain for OIDC discovery and token failures - ([ac9c8db](https://github.com/szinn/BookBoss/commit/ac9c8db0bb8a09e03f43f0681590979dca189408))

## [0.8.22](https://github.com/szinn/BookBoss/compare/v0.8.21..v0.8.22) - 2026-05-01

### Features

- _(bookboss)_ Wire OidcConfig through bookboss to frontend subsystem - ([af97287](https://github.com/szinn/BookBoss/commit/af97287da0d91930e5ff3e9c196436cc9d1c9c60))
- _(core)_ Add AuthService::is_valid_email for SSO callback - ([45ae2b2](https://github.com/szinn/BookBoss/commit/45ae2b29662c543c497fe5de95b2a5956b5b3f16))
- _(core)_ Add UserRepository::find_by_email - ([ed342b6](https://github.com/szinn/BookBoss/commit/ed342b62a90f299797c196f9076f54dff6cee687))
- _(frontend)_ Add INFO breadcrumbs through OIDC callback flow for debugging - ([4095f41](https://github.com/szinn/BookBoss/commit/4095f41fc9472d243dd75d6e66029dbedc52c7b9))
- _(frontend)_ Support login_failed query param on landing page - ([aa1a069](https://github.com/szinn/BookBoss/commit/aa1a069c32466ee144ab74dfaaa5ba7b4c3af149))
- _(frontend)_ Add SSO button to LoginForm with conditional rendering - ([1c46503](https://github.com/szinn/BookBoss/commit/1c46503b6c0b6c7a559fccc01e652f6c23daa12f))
- _(frontend)_ Wire OIDC routes into app router conditionally - ([c97ec4e](https://github.com/szinn/BookBoss/commit/c97ec4e69266b0bddf1d6bbe7e7573d5ae8cecee))
- _(frontend)_ Implement OIDC callback handler with token validation - ([e6c296f](https://github.com/szinn/BookBoss/commit/e6c296f81350c130e13f049f6e11f0b6a667f936))
- _(frontend)_ Implement OIDC start handler with PKCE - ([e0be38d](https://github.com/szinn/BookBoss/commit/e0be38d9930f7d3159cb3c3a79115558245a9289))
- _(frontend)_ Add OIDC handler module skeleton with discovery client - ([8058325](https://github.com/szinn/BookBoss/commit/80583258ead8a4bd57276d5e26771e8746f37680))
- _(frontend)_ Add OidcConfig struct with thiserror validation - ([96f9390](https://github.com/szinn/BookBoss/commit/96f93904db6ed9c29f7b46d204aee6b9f3a57791))
- _(frontend)_ Add openidconnect crate dependency for SSO - ([c981db1](https://github.com/szinn/BookBoss/commit/c981db1dc568ffa9af182053153a90ee67aa317f))

### Refactor

- _(frontend)_ Use server-side state store for OIDC flow instead of session cookie - ([01de8f7](https://github.com/szinn/BookBoss/commit/01de8f7bd991fcb165e6b29a53a3c1774c416485))
- _(frontend)_ Replace is_valid with is_sso_available, soft-fail on SSO failures - ([9e64934](https://github.com/szinn/BookBoss/commit/9e64934e9d8ce5017fdf8a16f88a1ce9bc480c56))

### Documentation

- Document OIDC SSO env vars and IdP setup in README - ([f40448c](https://github.com/szinn/BookBoss/commit/f40448cccb62382fbf26977deaf0ab8e6ca07bbe))

## [0.8.21](https://github.com/szinn/BookBoss/compare/v0.8.20..v0.8.21) - 2026-04-28

### Bug Fixes

- _(frontend)_ Add missing dark mode variants across UI components - ([653bb2a](https://github.com/szinn/BookBoss/commit/653bb2af37cd3f24e3a237f3a23139700c36698b))

### Miscellaneous Tasks

- _(config)_ Migrate Renovate config ([#127](https://github.com/szinn/BookBoss/issues/127)) - ([92bb2a5](https://github.com/szinn/BookBoss/commit/92bb2a5b07da1d7aa1c3c429de2ae0ce04f891ae))

## [0.8.20](https://github.com/szinn/BookBoss/compare/v0.8.19..v0.8.20) - 2026-04-28

### Features

- _(frontend)_ Dark mode variants for NavBar and AppLayout shell - ([9da5cd9](https://github.com/szinn/BookBoss/commit/9da5cd902dab5a6721fa475077e9ea6fbaafc5e9))
- _(frontend)_ Dark mode variants for profile page and shared components - ([c2780d3](https://github.com/szinn/BookBoss/commit/c2780d30ff1bfdef97f6b8cf7da0192fd3d872d3))
- _(frontend)_ Dark mode variants for settings page - ([2322369](https://github.com/szinn/BookBoss/commit/232236964a680967ea99172e58c564f45e92a87d))
- _(frontend)_ Dark mode variants for incoming and review pages - ([f8360c4](https://github.com/szinn/BookBoss/commit/f8360c4b2903d42231ce586004ff17aa1a05c63c))
- _(frontend)_ Dark mode variants for shelves page - ([412381e](https://github.com/szinn/BookBoss/commit/412381eae9fc801d89e5392e76db7c6be641a9ef))
- _(frontend)_ Dark mode variants for authors, series, and detail pages - ([5b16a9c](https://github.com/szinn/BookBoss/commit/5b16a9c2973a4bee049e5584b1fbdfe816a88e2d))
- _(frontend)_ Dark mode variants for book detail and edit metadata pages - ([56e1439](https://github.com/szinn/BookBoss/commit/56e1439d359e9a6057bd02e9266783e652916299))
- _(frontend)_ Dark mode variants for Books page and book cards - ([34c673b](https://github.com/szinn/BookBoss/commit/34c673bb333631e0e7cd22fbf36523979ba2cb61))
- _(frontend)_ Dark mode variants for LandingPage and auth forms - ([1e75243](https://github.com/szinn/BookBoss/commit/1e75243d7e3418c2c32336c6fb6cffb0680370b6))

### Bug Fixes

- _(frontend)_ Resolve dark mode theme toggle and persistence bugs - ([a8b210c](https://github.com/szinn/BookBoss/commit/a8b210c94340e63dd2db2241fda57e3aaa48a3ce))

### Miscellaneous Tasks

- _(frontend,metadata)_ Fix clippy warnings across frontend and metadata crates - ([68b7490](https://github.com/szinn/BookBoss/commit/68b749015ac59e6eeffbdc6cebcb6f8f089e186d))

## [0.8.19](https://github.com/szinn/BookBoss/compare/v0.8.18..v0.8.19) - 2026-04-25

### Bug Fixes

- _(frontend)_ Typo - ([9d6b0cc](https://github.com/szinn/BookBoss/commit/9d6b0cca329d630916888c7194032c78d8f092ca))

## [0.8.18](https://github.com/szinn/BookBoss/compare/v0.8.17..v0.8.18) - 2026-04-25

### Bug Fixes

- _(frontend)_ Allow larger uploads - ([54d420c](https://github.com/szinn/BookBoss/commit/54d420c6192e981c3fca40fc18ce67dba71cfd57))

## [0.8.17](https://github.com/szinn/BookBoss/compare/v0.8.16..v0.8.17) - 2026-04-24

### Bug Fixes

- _(core)_ Scope Kobo sync diff query by companion shelf library_id - ([b84f4ba](https://github.com/szinn/BookBoss/commit/b84f4baf4d30eabe7e765cc19a6b1adc14912f1b))
- _(core)_ Scope Kobo sync diff query by companion shelf library_id - ([4a651d6](https://github.com/szinn/BookBoss/commit/4a651d6dfc09216db3e289d71198341093704145))
- _(core)_ Resolve user default library when creating manual and smart shelves - ([c78459c](https://github.com/szinn/BookBoss/commit/c78459c54b7408176e9befe1b08d143d44bf5141))

## [0.8.16](https://github.com/szinn/BookBoss/compare/v0.8.15..v0.8.16) - 2026-04-18

### Bug Fixes

- _(core)_ Reset import job to Error when pipeline fails mid-flight - ([c7dd8fc](https://github.com/szinn/BookBoss/commit/c7dd8fc4a147d3f945dac37bfa17c183190f6381))
- _(core)_ Case-insensitive username lookup on login - ([0cd26ca](https://github.com/szinn/BookBoss/commit/0cd26ca53078dcbc19ea9f5bd45e0a0c4104a654))
- _(frontend)_ Refresh job queue badge after drag-and-drop upload - ([9bd3b02](https://github.com/szinn/BookBoss/commit/9bd3b0227523627b1c11381438c466edd36f312c))

## [0.8.15](https://github.com/szinn/BookBoss/compare/v0.8.14..v0.8.15) - 2026-04-17

### Bug Fixes

- _(frontend)_ Sort incoming imports by author then title - ([8ee3527](https://github.com/szinn/BookBoss/commit/8ee352749387e1158070b3a2c806cde4d77fe74f))

## [0.8.13](https://github.com/szinn/BookBoss/compare/v0.8.12..v0.8.13) - 2026-04-16

### Bug Fixes

- _(frontend)_ Raise axum DefaultBodyLimit to match RequestBodyLimitLayer - ([8e0e117](https://github.com/szinn/BookBoss/commit/8e0e1177855abc668c43382bce60232092ecf662))

## [0.8.12](https://github.com/szinn/BookBoss/compare/v0.8.11..v0.8.12) - 2026-04-16

### Bug Fixes

- _(api)_ Move system.proto to canonical buf layout and rename package to v1 - ([f9cca2c](https://github.com/szinn/BookBoss/commit/f9cca2c7778c57b69368f1c7d2f32d53baab1258))
- _(core)_ Accurately count and report reset import jobs in health check - ([8db5d77](https://github.com/szinn/BookBoss/commit/8db5d7720bd1eb48d3e3d0209edd183e42a7820e))
- _(database)_ Fix MySQL compatibility in migrations 37 and 38 - ([b16823c](https://github.com/szinn/BookBoss/commit/b16823cecb2aa12ba4d1dfaba2d02f8bdb7f6e99))

## [0.8.11](https://github.com/szinn/BookBoss/compare/v0.8.10..v0.8.11) - 2026-04-15

### Features

- _(core)_ Extend EnsureEnrichmentsHandler with MOBI backfill sweep - ([8625c2f](https://github.com/szinn/BookBoss/commit/8625c2f7618b7a937d1f2467ac5386f7cf387562))
- _(core)_ Enqueue MOBI conversion after book file enrichment - ([b2a3044](https://github.com/szinn/BookBoss/commit/b2a3044a278b80fe55a99e3877cc05e6b9602e4b))
- _(core)_ Add ConvertMobiHandler job handler - ([2282a8a](https://github.com/szinn/BookBoss/commit/2282a8afffc692055caac041099ae92c3394f8cc))
- _(core)_ Add app_setting domain - ([0f883c3](https://github.com/szinn/BookBoss/commit/0f883c3e1c47cab78135802f17d14a3ebd91957d))
- _(database)_ Add find_book_ids_needing_mobi_conversion query - ([56d8a8e](https://github.com/szinn/BookBoss/commit/56d8a8e3b4830ceec0075ea77032d5e20083c3f3))
- _(database)_ Implement AppSettingRepository SeaORM adapter - ([94558e0](https://github.com/szinn/BookBoss/commit/94558e006152f5af311a34ba7cc824310468d0eb))
- _(database)_ Add app_settings migration - ([ac9eb6a](https://github.com/szinn/BookBoss/commit/ac9eb6a34a632013acf4b248c4a1122751339aba))
- _(formats)_ Implement MOBI6 chapter anchor navigation - ([86c6be3](https://github.com/szinn/BookBoss/commit/86c6be3bd91017f32299f526b420a8e56b9ba085))
- _(formats)_ Add MOBI output branch to FormatService - ([c1ff040](https://github.com/szinn/BookBoss/commit/c1ff040a3a1fd5614a645f42caba7ca012e07f8d))
- _(formats)_ Add MOBI6 writer - ([c0c7b80](https://github.com/szinn/BookBoss/commit/c0c7b80cff0ecfdace0c9ae9b566ec16db82cbd3))
- _(frontend)_ Admin settings UI with MOBI toggle - ([06f7ea6](https://github.com/szinn/BookBoss/commit/06f7ea64582e97d9520b4b1162745f3ce1e1c21f))

### Bug Fixes

- _(core)_ Keep existing MOBI files up to date regardless of mobi_enabled flag - ([5d8ef61](https://github.com/szinn/BookBoss/commit/5d8ef61903913646f00c7abc20c37ac1865da522))
- _(formats)_ Collapse if guard into match arm pattern in NCX parser - ([ddd30ca](https://github.com/szinn/BookBoss/commit/ddd30ca355a50a8195744805c1572405ad7e7d87))
- _(formats)_ Use encode_number_as_hex for MOBI INDX entry keys - ([417956e](https://github.com/szinn/BookBoss/commit/417956eb24d0939754165dff47b7230ffc441241))
- _(frontend)_ Add cache-busting version param to download links - ([bfae035](https://github.com/szinn/BookBoss/commit/bfae035fbfb9b237c692212e4eb104cb6c159626))
- _(frontend)_ Reorder settings page nav sections - ([a007942](https://github.com/szinn/BookBoss/commit/a007942fa8bba098e95dacc90c0fb4f8bd389c4c))

## [0.8.10](https://github.com/szinn/BookBoss/compare/v0.8.9..v0.8.10) - 2026-04-15

### Features

- _(core)_ Implement queue_bytes_if_new - ([f4cd2c7](https://github.com/szinn/BookBoss/commit/f4cd2c7b9e83aaca11d0b5d92f0d205ad2025d4d))
- _(core)_ Add bookdrop_path to ImportJobServiceImpl - ([b000f07](https://github.com/szinn/BookBoss/commit/b000f07b69388f63cefc2727e754c64b84422ce1))
- _(frontend)_ Drag-and-drop EPUB upload on incoming page - ([a858bbc](https://github.com/szinn/BookBoss/commit/a858bbc5b50dca77c0b09fa2815f58e89098c664))
- _(frontend)_ Add upload_incoming_epub server function - ([167ad94](https://github.com/szinn/BookBoss/commit/167ad943e597616229798f5a6978d366b5690084))
- _(utils)_ Add hash_bytes function - ([8906fa3](https://github.com/szinn/BookBoss/commit/8906fa33a6622315ac7384dcf4e6190bb5dbae15))

### Bug Fixes

- _(frontend)_ Hide Add to Library button when no user libraries exist - ([b0ae006](https://github.com/szinn/BookBoss/commit/b0ae006a0400282d2f582f4b7bf49a158167c393))

## [0.8.9](https://github.com/szinn/BookBoss/compare/v0.8.8..v0.8.9) - 2026-04-14

### Bug Fixes

- _(frontend)_ Make NavBar responsive with two-row layout below sm breakpoint - ([7c62498](https://github.com/szinn/BookBoss/commit/7c624984a942dc8ec846275490a3d503f04289f7))
- _(frontend)_ Scope reading state query to displayed book IDs on shelf and author pages - ([68c61a9](https://github.com/szinn/BookBoss/commit/68c61a90e0f69f52c1e9d793a2b0cd92a81358fe))

## [0.8.8](https://github.com/szinn/BookBoss/compare/v0.8.7..v0.8.8) - 2026-04-12

### Bug Fixes

- _(api)_ Don't error on koreader get position if book is unread - ([88c28a1](https://github.com/szinn/BookBoss/commit/88c28a170ad0528177d04480b1c713d7968d8802))

## [0.8.7](https://github.com/szinn/BookBoss/compare/v0.8.6..v0.8.7) - 2026-04-12

### Bug Fixes

- _(core)_ Register KOReader partial MD5 hash for OPDS-downloaded books - ([17dfd69](https://github.com/szinn/BookBoss/commit/17dfd694dd1d3b93c67e7403924b7e3f19f3efe4))

## [0.8.6](https://github.com/szinn/BookBoss/compare/v0.8.5..v0.8.6) - 2026-04-11

### Bug Fixes

- _(frontend)_ Use PENDING_SEARCH to survive route-change clear - ([0d3e104](https://github.com/szinn/BookBoss/commit/0d3e104cadc9463c569d1e393f8a948d0923a543))

## [0.8.5](https://github.com/szinn/BookBoss/compare/v0.8.4..v0.8.5) - 2026-04-11

### Bug Fixes

- _(core)_ Fix md5 hex formatting for digest 0.11 Array type - ([aa14557](https://github.com/szinn/BookBoss/commit/aa14557fe4a6fc902ddc9030cb493911426289eb))
- _(frontend)_ Genre/tag links navigate with quoted exact search - ([1620203](https://github.com/szinn/BookBoss/commit/1620203f99ede6de3e2b8258a5b0a6a16414ba66))
- _(frontend)_ Quoted genre:/tag: field tokens use exact matching - ([cc128b8](https://github.com/szinn/BookBoss/commit/cc128b860c1e9cef203ad27a5b96473377a308d8))

## [0.8.4](https://github.com/szinn/BookBoss/compare/v0.8.3..v0.8.4) - 2026-04-04

### Bug Fixes

- _(frontend)_ Fix incoming queue cap — navbar badge and page list were both capped at 50 - ([00cf7a1](https://github.com/szinn/BookBoss/commit/00cf7a13742f8ca7689a76b98af79f9ebdff4c14))

## [0.8.3](https://github.com/szinn/BookBoss/compare/v0.8.2..v0.8.3) - 2026-04-04

### Bug Fixes

- _(database)_ Migrations don't need a down - ([4ed856a](https://github.com/szinn/BookBoss/commit/4ed856ab04817828e727048fa098ebe617f4233f))
- _(frontend)_ Fix incoming queue cap — navbar badge and page list were both capped at 50 - ([05ffc9e](https://github.com/szinn/BookBoss/commit/05ffc9e2c9e2a9961b61e4b6f63248ba84e6a15d))

## [0.8.2](https://github.com/szinn/BookBoss/compare/v0.8.1..v0.8.2) - 2026-04-04

### Features

- _(core)_ Add KoReaderService and document hash domain - ([93d8e15](https://github.com/szinn/BookBoss/commit/93d8e15570e017ff777d23036a3b0a983b13c3bd))
- _(database)_ Add koreader_document_hashes migration and adapter - ([002c7f1](https://github.com/szinn/BookBoss/commit/002c7f10916328bfed6db961e10515dba983fc38))
- _(frontend)_ Polish profile page OPDS and devices UI - ([951b54f](https://github.com/szinn/BookBoss/commit/951b54f8b7f75b0afa256a65f5b9f2471bf12529))
- _(frontend)_ Hide OPDS password by default with toggle - ([13796af](https://github.com/szinn/BookBoss/commit/13796af26c1e48f47a88a0fc0fa89eb78fb7ef23))
- _(frontend)_ Update profile OPDS section for KOReader - ([e205d75](https://github.com/szinn/BookBoss/commit/e205d758503dded6055cb2c570e5ae06d3e736c0))
- _(frontend)_ Register KOReader document hashes at download time - ([ef7b893](https://github.com/szinn/BookBoss/commit/ef7b893d84058e9c1cd52fad1effc6c33eb8dab9))
- _(frontend)_ Add KOReader sync endpoints under /koreader/ - ([e043705](https://github.com/szinn/BookBoss/commit/e043705587813f9bbc0e65371437891f007f4600))

### Bug Fixes

- _(frontend)_ Fix OPDS kepub exclusion and cover URL differentiation - ([d1fcb25](https://github.com/szinn/BookBoss/commit/d1fcb25601de07d9f395714c93d8946ff4689da5))

### Documentation

- _(libraries)_ All Books is an assignable library - ([1daa386](https://github.com/szinn/BookBoss/commit/1daa3863dee57e58fe43236b290a54709440443d))
- Add KOReader sync user guide - ([f2c3712](https://github.com/szinn/BookBoss/commit/f2c37120a23ef1873ae850bbd981d34fe2ec2b5d))

### Miscellaneous Tasks

- _(bookboss)_ Don't log dioxus_server::server messages - ([10bbef6](https://github.com/szinn/BookBoss/commit/10bbef6bbfb65f3653af830e2a05da44a276ec87))

## [0.8.1](https://github.com/szinn/BookBoss/compare/v0.8.0..v0.8.1) - 2026-04-04

### Features

- _(core)_ Extend count_for_filter to support manual shelves - ([d70e491](https://github.com/szinn/BookBoss/commit/d70e49154feaa4e55804096eef14761886f6264f))
- _(frontend)_ Populate book counts for all shelf bar pills - ([0a94d29](https://github.com/szinn/BookBoss/commit/0a94d2994e5f9f09815d283f7a79b3cdbe971d52))

### Bug Fixes

- _(frontend)_ Eliminate NavBar repaint flash on client-side navigation - ([77d0b54](https://github.com/szinn/BookBoss/commit/77d0b548aa1435e44afd18f7e2f4afcbf0f29372))
- _(frontend)_ Require authentication for job queue count endpoint - ([d89dafc](https://github.com/szinn/BookBoss/commit/d89dafcbb26284e086e88186c14934f5f1268130))
- _(frontend)_ Hide Currently Reading when browsing unowned libraries - ([660b291](https://github.com/szinn/BookBoss/commit/660b2916f9a7038e816b585d7d56f92dac9809d0))
- _(frontend)_ Fix two clippy warnings in nav_bar - ([36f7ec1](https://github.com/szinn/BookBoss/commit/36f7ec1ea9fa29acd98bb9c3c1addd0d1ac9b172))

### Refactor

- _(database)_ Make all migration down() impls no-ops - ([85f7860](https://github.com/szinn/BookBoss/commit/85f786009f49e44fdf5eb757342c4d3c1b123bec))
- _(database)_ Derive LibraryToken from id in From impl - ([c66c695](https://github.com/szinn/BookBoss/commit/c66c695e1e1bbf57899718a08e5d285823503b56))

### Documentation

- Document virtual libraries feature - ([54bb1cc](https://github.com/szinn/BookBoss/commit/54bb1cc073536cf776c6076e0b20d8c6639deda0))

### Performance

- _(core)_ Align sweep job priority and continuation delay - ([e86c8c0](https://github.com/szinn/BookBoss/commit/e86c8c0249215ab8c46922b3f58bf333b822d53e))
- _(core)_ Optimize backfill_thumbnails sweep scheduling - ([f1a4b11](https://github.com/szinn/BookBoss/commit/f1a4b116530c7c30e9591db330d2a2ee3e215771))

## [0.8.0](https://github.com/szinn/BookBoss/compare/v0.7.18..v0.8.0) - 2026-04-03

### Features

- _(core)_ Delete user's shelves on user deletion; add delete_shelves_for_user - ([7754234](https://github.com/szinn/BookBoss/commit/775423438e88c77b4985ded646806a5e511133b4))
- _(core)_ Seed personal library with user's shelf/metadata books only - ([75f1440](https://github.com/szinn/BookBoss/commit/75f1440450d9614cd07c546fcc4d539575d4bd0f))
- _(core)_ Auto-add approved books to All Books library in approve_book transaction - ([9e14573](https://github.com/szinn/BookBoss/commit/9e14573058495d8dd5e70a7af66f8ce504dbb89a))
- _(core)_ Add LibraryService trait, implementation, and CoreServices wiring - ([004c4ed](https://github.com/szinn/BookBoss/commit/004c4ed8f8532f334121a4654ae53d63ee374156))
- _(core)_ Add BookService::remove_unused_genres/tags - ([bdaefb6](https://github.com/szinn/BookBoss/commit/bdaefb67902d85d05d47b27a41243427706cd575))
- _(core,database)_ Add FilterRule::Library variant and library_condition builder - ([9ad5756](https://github.com/szinn/BookBoss/commit/9ad5756172e9f896e6f351a0902963f7c81f2b95))
- _(core,database,frontend)_ Add library_id scoping to book listing methods - ([c5531ee](https://github.com/szinn/BookBoss/commit/c5531ee4ad4bec4397f16da3fb9afd4e3f52351d))
- _(database)_ Add migration to drop shelves.visibility column and index - ([6a6deb2](https://github.com/szinn/BookBoss/commit/6a6deb2164da163a4dbbc5a816c7294284ee39fd))
- _(database)_ Fix list_libraries sort order and add find_by_token comment - ([df2e6dd](https://github.com/szinn/BookBoss/commit/df2e6dd027e5768c9b9d174fd13db886e442c855))
- _(database)_ Add SeaORM entities for library tables - ([77608c9](https://github.com/szinn/BookBoss/commit/77608c97c138db31dbe856b4cd761ae8911dfcbb))
- _(database)_ Add virtual library schema migrations - ([5b17465](https://github.com/szinn/BookBoss/commit/5b17465925015e4977685cbb9239da01b551d017))
- _(database)_ Add TagRepository::delete_unused_tags batch delete method - ([a2069e4](https://github.com/szinn/BookBoss/commit/a2069e46a87499bcd6f4104f3ffd703cc2ac3d5b))
- _(database)_ Add GenreRepository::delete_unused_genres batch delete method - ([d3626b6](https://github.com/szinn/BookBoss/commit/d3626b66111e2dfb187b6ff6259bfeeba0cefb40))
- _(frontend)_ Manage personal library from admin edit user modal - ([48ec42f](https://github.com/szinn/BookBoss/commit/48ec42f91f07a1e2cdfbc173a474f34a89bfbd75))
- _(frontend)_ Home button resets active library to user's default - ([2a93c8b](https://github.com/szinn/BookBoss/commit/2a93c8b8635c76b73d6a61c466ff3f8f6103656b))
- _(frontend)_ Add Libraries field to bulk edit modal and fix capability checks - ([26d9df0](https://github.com/szinn/BookBoss/commit/26d9df014099c3be5fcc9342ab1e577bee1db09c))
- _(frontend)_ Replace LibraryPicker select with custom dropdown - ([0ebe5bc](https://github.com/szinn/BookBoss/commit/0ebe5bcab73d930981b3157756958e8bfa75cae2))
- _(frontend)_ OPDS library-scoped feeds + drag-and-drop to Home/shelves - ([5c94b1a](https://github.com/szinn/BookBoss/commit/5c94b1a9c029489575add75d8777c4dbb5dcff76))
- _(frontend)_ Add Library filter rule + profile default library picker - ([46cc4bb](https://github.com/szinn/BookBoss/commit/46cc4bb4b2c4aee630b11c6e5a78488a07fc1b52))
- _(frontend)_ Add bulk 'Add to Library' action to selection bar - ([827dc9d](https://github.com/szinn/BookBoss/commit/827dc9d55b6a4d5495c9ac06305f9978b6d4fab1))
- _(frontend)_ Add library picker to edit metadata / incoming review - ([62d189f](https://github.com/szinn/BookBoss/commit/62d189f20239fc6256ab62ce7a70fc4e22282036))
- _(frontend)_ Add library assignment fields to Settings → Users forms - ([209dd03](https://github.com/szinn/BookBoss/commit/209dd034e55f95f3828014c5b3d0548d9acc3915))
- _(frontend)_ Add Libraries section to Settings page - ([95ef804](https://github.com/szinn/BookBoss/commit/95ef804f09c9468493261930b15bca3d1084910a))
- _(frontend)_ Scope book grid and search to ACTIVE_LIBRARY - ([fce7cd0](https://github.com/szinn/BookBoss/commit/fce7cd0cf0a956a91ab7353d8997ed4728b03d84))
- _(frontend)_ Add ACTIVE_LIBRARY signal and library picker to NavBar - ([d5f7cd1](https://github.com/szinn/BookBoss/commit/d5f7cd1b71265c2c9b90f569a51ccebba06cfd06))
- _(frontend)_ Add Remove unused button to Genre/Tags settings panels - ([8e91352](https://github.com/szinn/BookBoss/commit/8e913526949c2b3a8f47de1cdd60c4d39e4ef5b1))

### Bug Fixes

- _(core)_ Use user default library for device companion shelf - ([fbdd2e6](https://github.com/szinn/BookBoss/commit/fbdd2e649188582dcfa09241e3e8f811bd7c5092))
- _(frontend)_ Prevent All Books flash on browser refresh - ([a4f8845](https://github.com/szinn/BookBoss/commit/a4f88459a2e9d5abe008c2110f3a1b9924412719))
- _(frontend)_ Sort Libraries list — system first, then non-system, both alphabetical - ([6d1046a](https://github.com/szinn/BookBoss/commit/6d1046aacc820525d9425b6fdfc9597ea21f90cb))
- _(frontend)_ Include new personal library in assignment and set as default - ([645b32b](https://github.com/szinn/BookBoss/commit/645b32b01d4ba4d4eaea7efda384d9bef876721c))
- _(frontend)_ Fix Libraries table column alignment - ([d6a3ee7](https://github.com/szinn/BookBoss/commit/d6a3ee73172d8e5572e8c4110203f42b25a4047a))
- _(frontend)_ Always show Create personal library in user edit form - ([d447db9](https://github.com/szinn/BookBoss/commit/d447db92bd7506da7d8bb090e9f6ee1d0198f11e))
- _(frontend)_ Auto-fill personal library name when checkbox checked in edit mode - ([8b475de](https://github.com/szinn/BookBoss/commit/8b475de1214565ecec640753614c2476e88e0f62))
- _(frontend)_ Allow saving SuperAdmin users in user admin settings - ([c24e8fc](https://github.com/szinn/BookBoss/commit/c24e8fccd2a9e6680bc1f8900102e40c043659b2))

### Refactor

- _(core)_ Remove ShelfVisibility — all shelves are owner-only - ([b87fd89](https://github.com/szinn/BookBoss/commit/b87fd8963cbd2d5908d3d190a85adff53d622814))
- _(core)_ Rename LibraryService/Repository to CollectionService/Repository - ([f8d99af](https://github.com/szinn/BookBoss/commit/f8d99af3b674b2bb8e3a9b60986a83d3b9308dcc))
- _(database)_ Remove ShelfVisibility from entity, adapter, and integration tests - ([dd4fdd7](https://github.com/szinn/BookBoss/commit/dd4fdd707cb0e73063121009b54df6c47b4fd59d))
- _(database)_ Import ALL_BOOKS_LIBRARY_TOKEN from bb_core in migrations - ([4a9de2a](https://github.com/szinn/BookBoss/commit/4a9de2ab0a04d40375ad5797526cceb2bb2eeaed))
- _(frontend)_ Remove ShelfVisibility from server fns and UI - ([7cf0b35](https://github.com/szinn/BookBoss/commit/7cf0b354b935a541dd97360e6c78580ca9a7b44f))

## [0.7.18](https://github.com/szinn/BookBoss/compare/v0.7.17..v0.7.18) - 2026-04-01

### Features

- _(core)_ Add BookService::fetch_hydration_data - ([167a7b7](https://github.com/szinn/BookBoss/commit/167a7b7434069d5d71150f0bf04c41f959b7a3bd))
- _(core)_ Add BookHydrationData + batch repository trait methods - ([b48f7ba](https://github.com/szinn/BookBoss/commit/b48f7ba89aa03233e65402e3818c723f5818dd1d))
- _(database)_ Implement batch repository methods for hydration - ([0e97b8e](https://github.com/szinn/BookBoss/commit/0e97b8eaaca15e2b47e681638033442ebd7bbe95))

### Refactor

- _(frontend)_ Consolidate validate_password_strength into password module - ([4ec5e31](https://github.com/szinn/BookBoss/commit/4ec5e31f8d86d28bec03868650b788dc9d22abba))

### Performance

- _(frontend)_ Replace hydrate_books N+1 loops with batch queries - ([934a0ff](https://github.com/szinn/BookBoss/commit/934a0ff18b3cd35207469ed2ef77559b993fa484))

## [0.7.17](https://github.com/szinn/BookBoss/compare/v0.7.16..v0.7.17) - 2026-04-01

### Features

- _(core)_ Replace cover_path Option<String> with has_cover bool - ([975b1d6](https://github.com/szinn/BookBoss/commit/975b1d63c4e6a904031a61020625b02b0720309a))
- _(core)_ Add BackfillThumbnailsHandler health task - ([dd4f9bd](https://github.com/szinn/BookBoss/commit/dd4f9bdafff4671ccaa385684804f8972a791f10))
- _(core)_ Add find_book_ids_with_cover_for_sweep to BookRepository - ([288281a](https://github.com/szinn/BookBoss/commit/288281a754770c75964339f391a37864e41f8832))
- _(database)_ Add migration to replace cover_path with has_cover bool - ([8e96faf](https://github.com/szinn/BookBoss/commit/8e96fafb63c857c0a22155da64fd2b798e4d485a))
- _(frontend)_ Add ?full=1 to cover endpoint for single-book views - ([ac6d18c](https://github.com/szinn/BookBoss/commit/ac6d18ca013bf20d44e5f75affc94c5c694b383a))
- _(frontend)_ Cap book grid covers at 256px width - ([6b97f1b](https://github.com/szinn/BookBoss/commit/6b97f1bd848f234aa6fef0dd8dae864f274a80b1))
- _(frontend)_ Serve thumb.jpg first with fallback to cover.jpg - ([e720fda](https://github.com/szinn/BookBoss/commit/e720fda6aa84c1b45943b5c6e715069b0abc088a))
- _(storage)_ Add backfill_thumbnail to FileStoreService port - ([114e458](https://github.com/szinn/BookBoss/commit/114e45873637dbff857e0c547ce7dc3ae8084e41))
- _(storage)_ Generate 256x384 thumb.jpg alongside cover at ingest - ([6eccce6](https://github.com/szinn/BookBoss/commit/6eccce696a4b7923c6c2ed83026c6dbd348719f4))

### Bug Fixes

- _(frontend)_ Use full=true instead of full=1 for cover endpoint - ([883b62f](https://github.com/szinn/BookBoss/commit/883b62f1870180ec320791020e1717af9c695f42))
- _(frontend)_ Add missing OpdsAccess capability to user modal - ([d0d2570](https://github.com/szinn/BookBoss/commit/d0d25701f0b5ec3eac164a8b63397a4eeee0ae1a))

### Refactor

- _(core)_ Encapsulate cover/thumbnail filenames in storage crate - ([c630066](https://github.com/szinn/BookBoss/commit/c6300668d03e5b3c6493913395019b8840631cbf))
- _(frontend)_ Update callers to new FileStoreService API - ([30f7a94](https://github.com/szinn/BookBoss/commit/30f7a94b38298e1ab4ad9a82cc11b034cdc606da))

## [0.7.16](https://github.com/szinn/BookBoss/compare/v0.7.15..v0.7.16) - 2026-04-01

### Bug Fixes

- _(core)_ Use human-readable names in enrichment and stale-job system messages - ([9a7179f](https://github.com/szinn/BookBoss/commit/9a7179fe2a6eb82dd1b25d1de2038098d4be388f))
- _(core)_ Active recovery in VerifyFileIntegrityHandler + fix KEPUB download blank screen - ([5c1902a](https://github.com/szinn/BookBoss/commit/5c1902af007ab7ea1f1dce2e9783c7598eeb2ea1))
- _(frontend)_ Clear search text when clicking top-level nav links - ([c01287c](https://github.com/szinn/BookBoss/commit/c01287cc7aa4abd5c32e318679de71c5c83c6fe7))
- _(frontend)_ Show error and disable Approve on review page when original file missing - ([a6cf27e](https://github.com/szinn/BookBoss/commit/a6cf27ef3b54d2fdd5a0e9c9ad52d8fe80d4ee62))

### Refactor

- _(core)_ Standardize owner-check errors in DeviceService - ([4b3fe19](https://github.com/szinn/BookBoss/commit/4b3fe19f36d0ef8f8b9534e68f89fd5e3db3fd3d))
- _(core)_ Extract normalize_name to bb-utils - ([8957412](https://github.com/szinn/BookBoss/commit/8957412e16d75060460260620b4cbb419f9e0eca))
- _(formats)_ Replace string OPF manipulation with quick_xml streaming - ([e66ddf5](https://github.com/szinn/BookBoss/commit/e66ddf5717b4ae8c4c5a1b4512775800652d0fbf))
- _(formats)_ Add FormatService::read_raw_opf, demote epub visibility - ([f2a63da](https://github.com/szinn/BookBoss/commit/f2a63da519506b7fb5d540749be5b30b6fe162ec))
- _(formats)_ Extract shared test utilities - ([a44f075](https://github.com/szinn/BookBoss/commit/a44f0751bae38f61c90812f1cd4a10efecc6dc36))
- _(frontend)_ Consolidate password validation into crate::password - ([3a81698](https://github.com/szinn/BookBoss/commit/3a81698b8997b9dbd594cb0f023de0f56bfce95e))
- _(frontend)_ Extract require_capability helper - ([736137c](https://github.com/szinn/BookBoss/commit/736137c64e38ab765c47a131f7601f3ea067ec9e))
- _(frontend)_ Extract to_server_err helper - ([56718ee](https://github.com/szinn/BookBoss/commit/56718ee3d9b6f2514a2498ee8f4ef53ff6fe8ddb))
- _(metadata)_ Consolidate OL author-list building into helper - ([0b716fa](https://github.com/szinn/BookBoss/commit/0b716fa7af0380bb9a91c4bd91018dcb5302fff7))
- _(metadata)_ Extract parse_year to bb-utils - ([cfd805c](https://github.com/szinn/BookBoss/commit/cfd805c3fe31a1071d54f3665806eb40a34a3760))
- _(storage)_ Log JPEG encoding fallback + extract ensure_dir helper - ([ea2364f](https://github.com/szinn/BookBoss/commit/ea2364f9eae8046f229ed93aba0c7e321627917d))

### Testing

- _(core)_ Add library service guard tests - ([c226f80](https://github.com/szinn/BookBoss/commit/c226f80babb535f4764fbcd6e38407e58eda6bc8))
- _(core)_ Add missing reading service tests - ([81b9c2f](https://github.com/szinn/BookBoss/commit/81b9c2f6bbe36b63d68725663dbb78626d928cef))
- _(core)_ Add pipeline pure-function tests - ([ddc1b80](https://github.com/szinn/BookBoss/commit/ddc1b80c550b99988757eb6978c11d23fd872044))
- _(formats)_ Add missing test coverage for parse and write modules - ([6d35640](https://github.com/szinn/BookBoss/commit/6d356405d544687be76f1dc48e4d2dcf82227197))
- _(frontend)_ Add review page pure function tests - ([3f9f200](https://github.com/szinn/BookBoss/commit/3f9f200fdbcefb95ca09abb1d488f75a10e99ff8))
- _(metadata)_ Add error-path tests to all three adapters - ([27cf26c](https://github.com/szinn/BookBoss/commit/27cf26c339777545cbbddbd162151726822a0984))
- _(storage)_ Add missing storage operation tests - ([5fa1365](https://github.com/szinn/BookBoss/commit/5fa1365f4b8491de86eec2d32484d99f02080001))
- _(storage)_ Add normalize_to_jpeg unit tests - ([7dc50b0](https://github.com/szinn/BookBoss/commit/7dc50b06ae284187264438f7319be7ebf5945d1d))

## [0.7.15](https://github.com/szinn/BookBoss/compare/v0.7.14..v0.7.15) - 2026-03-31

### Features

- _(database)_ Add missing database indices - ([a29d973](https://github.com/szinn/BookBoss/commit/a29d9737a638c4b9111a656cf2c9068998d9d3d2))
- _(frontend)_ Extend tab-completion to cycle status: field values - ([ac1c693](https://github.com/szinn/BookBoss/commit/ac1c6939dc3821f4ccb7afeb21bb4e95ae009219))
- _(frontend)_ Add tab-completion ghost text for search field keywords - ([d75ff35](https://github.com/szinn/BookBoss/commit/d75ff356747363be3434a75cdfc9332e2f29f6db))
- _(frontend)_ Add rotating placeholder tips for search field - ([a6c5f07](https://github.com/szinn/BookBoss/commit/a6c5f07567a2c576b7dcdbea76207600be2775ab))
- _(frontend)_ Add search ? button and first-visit hint strip - ([9f1a91f](https://github.com/szinn/BookBoss/commit/9f1a91f9972995b49663071347b9ae2aee34c2cd))

### Bug Fixes

- _(frontend)_ Bust cover image cache after replacement using updated_at - ([64cf538](https://github.com/szinn/BookBoss/commit/64cf538265f40925a2c4e579e1ea2df507e6d0aa))
- _(frontend)_ Make ? button toggle hint strip instead of only opening it - ([0c16c30](https://github.com/szinn/BookBoss/commit/0c16c3006279fb53866606a9c55e8fbf125c24a8))

### Refactor

- _(core)_ Eliminate handler type repetition in before_start - ([7827d41](https://github.com/szinn/BookBoss/commit/7827d4165ee3c77fda127459e314cd1b0391e7e9))
- _(core)_ Make bookdrop_path required and scan_interval non-optional in ExternalServices - ([8a012e8](https://github.com/szinn/BookBoss/commit/8a012e85382f48446f36e317d923e57268b6ecfb))
- _(database)_ Extract pagination constants and lower_name_eq helper - ([aa00629](https://github.com/szinn/BookBoss/commit/aa0062903c5e912f6e3c3c886593236e224c178c))

### Testing

- _(database)_ Add missing tests for library and system_message adapters - ([58dfb0d](https://github.com/szinn/BookBoss/commit/58dfb0d07e55850d25198c3bdb828249f51bcd06))

## [0.7.14](https://github.com/szinn/BookBoss/compare/v0.7.13..v0.7.14) - 2026-03-30

### Features

- _(core)_ Add transient DB error retry in JobWorker poll loop - ([315bf64](https://github.com/szinn/BookBoss/commit/315bf641ad97ca72aecfb0543c83e0a205eac908))
- _(core)_ Implement CheckedSubsystem for CoreSubsystem with worker keepalive - ([619aea3](https://github.com/szinn/BookBoss/commit/619aea333aa450f48e8e00c1262694db3e153e6a))
- _(core)_ Add CheckedSubsystem trait and ResilienceWrapper - ([a52e36c](https://github.com/szinn/BookBoss/commit/a52e36c995acd220d2ea23cf37a521cf764db4e8))
- _(core)_ Add error classification for transient infrastructure failures - ([1c56a30](https://github.com/szinn/BookBoss/commit/1c56a3082b8fb8aec0629a684c2ff2e48e8e0447))
- _(core,database)_ Add Repository::ping() for connectivity checks - ([8ecf29b](https://github.com/szinn/BookBoss/commit/8ecf29bf137e2fa9ea1fa8efe5538b2556c59c1c))
- _(frontend)_ Return HTTP 503 on transient DB errors in cover and download endpoints - ([d714365](https://github.com/szinn/BookBoss/commit/d7143658aef63ab9300fd0f5230d85a49eb4f044))

### Bug Fixes

- _(core)_ Move path.exists() check behind FileStoreService port - ([68c3772](https://github.com/szinn/BookBoss/commit/68c377290861ba5989c47600f2699053859f6736))
- _(core)_ Move import job to Error immediately when file is missing - ([e537c8e](https://github.com/szinn/BookBoss/commit/e537c8ed39ee2166412e22d26a2786d97f940aaa))

## [0.7.13](https://github.com/szinn/BookBoss/compare/v0.7.12..v0.7.13) - 2026-03-30

### Features

- _(core)_ Move rejected bookdrop files to Trash/Bookdrop/ instead of deleting - ([95d06a6](https://github.com/szinn/BookBoss/commit/95d06a6ab8233ab53565931ac08b7bdadd6bc04b))

### Bug Fixes

- _(core)_ Reject non-EPUB formats at the bookdrop scanner - ([49745c5](https://github.com/szinn/BookBoss/commit/49745c5793941b8b28f9d3c9fa9dbb8508194d4d))
- _(core)_ Prevent scanner from deleting in-flight pipeline source file - ([5d9ba2d](https://github.com/szinn/BookBoss/commit/5d9ba2de4c6bd79397004fdf10344262919c87fb))
- _(frontend)_ Clear search bar on route change - ([8e87d04](https://github.com/szinn/BookBoss/commit/8e87d04cfdc7deae7b15685d6337b74d93e0299d))

## [0.7.12](https://github.com/szinn/BookBoss/compare/v0.7.11..v0.7.12) - 2026-03-30

### Features

- _(core)_ Add ReconcileFingerprintsHandler health task - ([8539dd7](https://github.com/szinn/BookBoss/commit/8539dd7fcba8d388edbfcbb3a0bb728443194c3c))
- _(core)_ Compute and store sidecar fingerprint after enrichment - ([7bbd203](https://github.com/szinn/BookBoss/commit/7bbd203569ea5adae007512fceb67d29fec31434))
- _(core)_ Null sidecar fingerprint in delete_genre and delete_tag - ([181a196](https://github.com/szinn/BookBoss/commit/181a19695fb324d6e2372a778f477acfdeadb229))
- _(core)_ Add sidecar fingerprint function and repository method - ([737d6cd](https://github.com/szinn/BookBoss/commit/737d6cd014cf2df154037f50e1bc35bcc000cc43))
- _(core)_ Add sidecar fingerprint migration and manual-only health task support - ([4598f57](https://github.com/szinn/BookBoss/commit/4598f5706410154422fe17f5cbbbad8b84de94f6))

### Testing

- _(database)_ Add component tests for sidecar fingerprint - ([e34b519](https://github.com/szinn/BookBoss/commit/e34b5197789dbb4d9b87e475d5123bf808528ff4))

## [0.7.11](https://github.com/szinn/BookBoss/compare/v0.7.10..v0.7.11) - 2026-03-30

### Bug Fixes

- _(database)_ Fix two SQLite integration test failures - ([2ba8c7f](https://github.com/szinn/BookBoss/commit/2ba8c7fc306bb6df96728cb36d7f7769c4680ae7))

## [0.7.10](https://github.com/szinn/BookBoss/compare/v0.7.9..v0.7.10) - 2026-03-29

### Features

- _(core)_ Convert EnsureEnrichmentsHandler to BookIdSweep cursor pattern - ([a1b047c](https://github.com/szinn/BookBoss/commit/a1b047cea53c1ff6e0bedbe124cac93189610264))
- _(core)_ Add BookIdSweep trait, runner, and find_available_books_for_sweep - ([8246971](https://github.com/szinn/BookBoss/commit/8246971ddd9d6e1f46d317ad714f235db8bad438))
- _(core)_ Add enqueue_after API for delayed job scheduling - ([414fc2c](https://github.com/szinn/BookBoss/commit/414fc2c83c667218eb9103a2323df75f113a70ef))
- _(core)_ Formalize job priority constants - ([edd3a72](https://github.com/szinn/BookBoss/commit/edd3a72f1b9bf33ab27606048ef894f8b5dd646f))

### Bug Fixes

- _(core)_ Enqueue enrichment immediately after replace_cover - ([3076bc9](https://github.com/szinn/BookBoss/commit/3076bc93591365a1e94fea9380b87eeea48ba372))
- _(core)_ Enqueue enrichment for affected books when genre/tag deleted - ([6a7a149](https://github.com/szinn/BookBoss/commit/6a7a149f3198165653c510e9dcf38f425bed5c62))
- _(core)_ Fix scanner dedup for incoming-queue and rejected files - ([84308d5](https://github.com/szinn/BookBoss/commit/84308d58048c233bcb1d3288f2b01a14669a6aa3))
- _(frontend)_ Focus delete button on every modal open in genre/tag settings - ([1e5f50b](https://github.com/szinn/BookBoss/commit/1e5f50b430b199807b3169899877724822ab80fa))

## [0.7.9](https://github.com/szinn/BookBoss/compare/v0.7.8..v0.7.9) - 2026-03-29

### Features

- _(core)_ Post system message and remove duplicate bookdrop files - ([027d047](https://github.com/szinn/BookBoss/commit/027d0474d9765f5db17dd79e91053bf7403b4295))
- _(core)_ Return FileQueueStatus from queue_file_if_new - ([170ee9c](https://github.com/szinn/BookBoss/commit/170ee9c80d152ea4416a9f5e607056fc75266933))

### Bug Fixes

- _(core)_ Add error context and system message to EnrichBookFilesHandler - ([c1fec31](https://github.com/szinn/BookBoss/commit/c1fec3106c56a238bcec1370d3943eeee147c7a3))
- _(core)_ Check book_files before import_jobs in scanner dedup - ([f62cd96](https://github.com/szinn/BookBoss/commit/f62cd969854b0630707c06aae78c54b6a9c43c5e))

### Miscellaneous Tasks

- _(core)_ Mark bug-bad-log-enrich-book-files-job complete - ([be5532f](https://github.com/szinn/BookBoss/commit/be5532ff87638146d7ca715f487a60f3d394aae5))
- _(core)_ Triage bug-bad-log-enrich-book-files-job - ([fb6aa2b](https://github.com/szinn/BookBoss/commit/fb6aa2b7015c13843c7db73910ac7f16569941a0))

## [0.7.8](https://github.com/szinn/BookBoss/compare/v0.7.7..v0.7.8) - 2026-03-29

### Features

- _(frontend)_ Add arrow-key navigation to ChipInput dropdown - ([dfa40da](https://github.com/szinn/BookBoss/commit/dfa40da38e5f2af8e894991f3d1a8385664b0b87))

### Bug Fixes

- _(frontend)_ Fix provider cover overwriting user-staged cover in review/edit - ([6e5b08d](https://github.com/szinn/BookBoss/commit/6e5b08d3a1991b127aa78b9adabfb560dd9657ed))
- _(frontend)_ Make Delete the default action in genre/tag confirm dialog - ([402e8a2](https://github.com/szinn/BookBoss/commit/402e8a221f3f78197fcedb6f9a188f25b66f3dbd))
- _(frontend)_ Freshen stale entity labels when opening smart shelf editor - ([ebbb4a2](https://github.com/szinn/BookBoss/commit/ebbb4a258d01e041a6c74098e2109847f090ea36))
- _(frontend)_ Fix single-value picklist UX and add keyboard nav to AutocompleteInput - ([a020a28](https://github.com/szinn/BookBoss/commit/a020a28b43e1d613b6cc27cfb100d30008bf918b))
- _(frontend)_ Remove 8-item cap from ChipInput dropdown - ([0ba776f](https://github.com/szinn/BookBoss/commit/0ba776fb7fb21d454733b1674df59fe5f2be4e85))
- _(frontend)_ Skip force-password-change when admin edits own account - ([21d6387](https://github.com/szinn/BookBoss/commit/21d63873a570ca9bf4d54b8f64488405b6c04902))
- _(frontend)_ Allow currently reading section to scroll with library - ([f292ce8](https://github.com/szinn/BookBoss/commit/f292ce85d759d4a29299d39e66140ecabec06d13))

### Refactor

- _(core)_ Centralise language normalisation in bb-utils - ([f3aa5a7](https://github.com/szinn/BookBoss/commit/f3aa5a7dce171e9209edbf9b5ec64a67c19a738b))
- _(core)_ Reduce visibility of CoreServices - ([90fecbd](https://github.com/szinn/BookBoss/commit/90fecbd9a378313ae8fe4de57a42bc3716ab2f02))

## [0.7.7](https://github.com/szinn/BookBoss/compare/v0.7.6..v0.7.7) - 2026-03-29

### Bug Fixes

- _(frontend)_ Keep NavBar and ShelfBar sticky during scroll - ([b4d4edc](https://github.com/szinn/BookBoss/commit/b4d4edcef6cc2a09147a201800cec246d6e4d64b))
- _(frontend)_ Fix missing read status badges on series/books pages - ([e3bf7bc](https://github.com/szinn/BookBoss/commit/e3bf7bcc99ebdf60de9dba111db0c47261f4e3af))

## [0.7.6](https://github.com/szinn/BookBoss/compare/v0.7.5..v0.7.6) - 2026-03-28

### Bug Fixes

- _(database)_ Remove 50-book limit from list_books - ([302ff43](https://github.com/szinn/BookBoss/commit/302ff43099b797f358517a424dffe71cb2a7d3c7))

## [0.7.5](https://github.com/szinn/BookBoss/compare/v0.7.4..v0.7.5) - 2026-03-28

### Features

- _(core)_ Add LibraryService::replace_cover - ([e43e43d](https://github.com/szinn/BookBoss/commit/e43e43d7bf4f5db084b1f5a6ee64208527d9336c))
- _(frontend)_ Add status: field to book search - ([63659c1](https://github.com/szinn/BookBoss/commit/63659c12e726155c6914f7f1e351eb6c9c019477))
- _(frontend)_ Drag-and-drop cover replacement on review and edit screens - ([bb0370b](https://github.com/szinn/BookBoss/commit/bb0370b2c7c297457f0817a154a92f771dc5a571))
- _(storage,core)_ Normalize all covers to JPEG regardless of source format - ([6bd6f6a](https://github.com/szinn/BookBoss/commit/6bd6f6a3f59448012d65b770dd0a999bb92e4b8b))

### Bug Fixes

- _(frontend)_ Stage dropped cover in temp dir; commit only on Save/Approve - ([750fe4b](https://github.com/szinn/BookBoss/commit/750fe4b4985156046dff5022f6a400291d8d841d))

## [0.7.4](https://github.com/szinn/BookBoss/compare/v0.7.3..v0.7.4) - 2026-03-28

### Features

- _(core)_ Add Shelf filter rule variant - ([dabc0c9](https://github.com/szinn/BookBoss/commit/dabc0c93ed185a91ecc7f8555572bb4467365d9d))
- _(core,database)_ Add delete/create/list_with_counts for genres and tags - ([35d1ca1](https://github.com/szinn/BookBoss/commit/35d1ca1f96644b77503da7fd99e7d5dd31e3ae52))
- _(core,database,frontend)_ Available-only counts, incoming badge, clickable genre/tag names - ([a9d28ff](https://github.com/szinn/BookBoss/commit/a9d28ff3f15ec41cdb4eed23b86e69da89d55e61))
- _(frontend)_ Send series metadata in Kobo sync responses - ([d904cd7](https://github.com/szinn/BookBoss/commit/d904cd7212a0d901410a1dc66b4816e1e3236abe))
- _(frontend)_ Move Genre/Tags nav item to second position in settings sidebar - ([c7c1d3e](https://github.com/szinn/BookBoss/commit/c7c1d3edd0de9cf0fc5b0b0a6478c7c96f2f9d8b))
- _(frontend)_ Add Genre/Tags settings section with create/delete for genres and tags - ([b1ba442](https://github.com/szinn/BookBoss/commit/b1ba44218e253f1d6230ffe70af2349efb12c8c0))
- _(frontend)_ Add Shelf filter rule UI and populate shelves in FilterEntityOptions - ([7f4f741](https://github.com/szinn/BookBoss/commit/7f4f7417316fc40d10e947f693f9ec76dbff5540))

### Bug Fixes

- _(frontend)_ Refresh books after bulk status change on author/series pages - ([43c12ac](https://github.com/szinn/BookBoss/commit/43c12ac2e47f59ce2c49e6fa0f03bdd6eb06111d))

## [0.7.3](https://github.com/szinn/BookBoss/compare/v0.7.2..v0.7.3) - 2026-03-28

### Miscellaneous Tasks

- _(docker)_ Pin dioxus-cli install to version 0.7.3 - ([754f4a1](https://github.com/szinn/BookBoss/commit/754f4a1874dfab28cc4441bd32594996a599307c))

## [0.7.2](https://github.com/szinn/BookBoss/compare/v0.7.1..v0.7.2) - 2026-03-28

### Bug Fixes

- _(core,database)_ Fill in reviewed_by on approve_book - ([9c364e2](https://github.com/szinn/BookBoss/commit/9c364e251f2ea4abeac3eaa5fd2f03aa00d9202a))
- _(core,utils)_ Adapt to sha2 0.11 breaking API changes - ([828437b](https://github.com/szinn/BookBoss/commit/828437b3a6cf60a34ecda4ed735b0af5d9377e5d))
- _(frontend)_ Show descriptive sort button tooltips - ([f239582](https://github.com/szinn/BookBoss/commit/f239582effca824f5f9331d9e7e50c625a003473))
- _(frontend)_ Exclude incoming books from series and authors pages - ([f952cb5](https://github.com/szinn/BookBoss/commit/f952cb5ae13ee0c18eab0ed3976984cf2ab26986))
- _(frontend)_ Sort incoming books by title - ([7da9873](https://github.com/szinn/BookBoss/commit/7da98737af4d19de184dfd1ce5d28a6eaa26b8f5))
- _(frontend)_ Refresh shelf counts after bulk operations - ([d9465cf](https://github.com/szinn/BookBoss/commit/d9465cfb61ca4de821f5d0b7214bcea4abda4b5b))
- _(frontend)_ Fix search bar shifting when nav badge counts change - ([0465c28](https://github.com/szinn/BookBoss/commit/0465c28ab6188652d5915b1bfeecd2cb90febc99))
- _(frontend)_ Decorate book tiles with reading state on series/author pages - ([2a54785](https://github.com/szinn/BookBoss/commit/2a5478506a86c3d7dfe70c40191a1d0ba7f7b181))
- _(metadata)_ Normalize Google Books language tags to root subtag - ([8bd6ac9](https://github.com/szinn/BookBoss/commit/8bd6ac9789906c26ce2089349ba05b3785326476))

### Refactor

- _(core)_ Hide scan channel wiring inside import module - ([8e82987](https://github.com/szinn/BookBoss/commit/8e82987ca72a10265f0425c75f6f38aa3bf41d8b))
- _(core)_ Route health job enqueueing through JobService - ([9c065ce](https://github.com/szinn/BookBoss/commit/9c065cef5afdfd423f8ccc0a641125575f14af0b))
- _(core)_ Move approve/reject/edit from PipelineService to LibraryService - ([d3b8ec5](https://github.com/szinn/BookBoss/commit/d3b8ec5d645b31766bf5e7d5a3025dbe56d4dd7d))
- _(core,frontend)_ Move fetch_from_provider to MetadataService - ([32d78dd](https://github.com/szinn/BookBoss/commit/32d78dd10ad72756c8ed6220eafff8012978b34c))
- _(core,metadata)_ Promote MetadataProvider to its own domain - ([159f844](https://github.com/szinn/BookBoss/commit/159f8441194b2c74d52d961d1c0381cce2fc0873))

### Miscellaneous Tasks

- _(release)_ Use jj git push for the bookmark - ([b3126a7](https://github.com/szinn/BookBoss/commit/b3126a733187d09b0ad1c40497f3a541fca592ca))

## [0.7.1](https://github.com/szinn/BookBoss/compare/v0.7.0..v0.7.1) - 2026-03-25

### Features

- _(frontend)_ Center format column and auto-scan on incoming page entry - ([137a430](https://github.com/szinn/BookBoss/commit/137a430282d86d545469f0f39cce7a29af7aea0d))

### Bug Fixes

- _(core)_ Fix integration test mock for enrich_book_files job enqueue - ([06bbc59](https://github.com/szinn/BookBoss/commit/06bbc59a2096fe6df20d3d4754a8f3b30971aed3))
- _(core)_ Clean up stale anonymous sessions in addition to expired ones - ([35f663c](https://github.com/szinn/BookBoss/commit/35f663c95b806f4aa3e6da2bdff97b4e2f22e74f))
- _(frontend)_ Clear search bar on Escape key press - ([f315c59](https://github.com/szinn/BookBoss/commit/f315c59c530c0cc35a518a0bda7890a051d4936f))

### Refactor

- _(cli)_ Remove bb-import dependency from bookboss - ([2759562](https://github.com/szinn/BookBoss/commit/275956298c4ddcfcff718330ea9616a1e4af1f42))
- _(cli)_ Update binary wiring for FormatService - ([9b019b9](https://github.com/szinn/BookBoss/commit/9b019b9c4e4048487a1b737b1cfcf5ad657fab3c))
- _(cli)_ Replace manual handler registration with before_start() calls - ([9fd80c7](https://github.com/szinn/BookBoss/commit/9fd80c78095308e844cb256579d4525e025f6f60))
- _(core)_ Make import job file_format non-nullable, detect at scan time - ([cc12c47](https://github.com/szinn/BookBoss/commit/cc12c47b01a23a1727f477e4d011160d186acd15))
- _(core)_ Delete crates/import, clean up workspace references - ([16035ec](https://github.com/szinn/BookBoss/commit/16035ecdd0cfe77268a115682d3be4ed87d455ed))
- _(core)_ Move ImportSubsystem into CoreSubsystem, wire scan channel - ([cee911b](https://github.com/szinn/BookBoss/commit/cee911b6452d3a1107cbb5126a8641d107456605))
- _(core)_ Move ProcessImportHandler into core import domain - ([ff41fcf](https://github.com/szinn/BookBoss/commit/ff41fcff8f7297264230627bb0dbeeb9ad69f527))
- _(core)_ Move scanner into core import domain, make file_format optional - ([f67e5a6](https://github.com/szinn/BookBoss/commit/f67e5a612016bebcc459ea877d15aa0da033ee65))
- _(core)_ Add list_files to FileStoreService port - ([ca55c08](https://github.com/szinn/BookBoss/commit/ca55c08cb88e98ae476829b74317b54eeacd43ba))
- _(core)_ Delete deprecated conversion/extractor modules and formats handlers - ([19328c7](https://github.com/szinn/BookBoss/commit/19328c71969d1ab8639d84f8933973aa76ef2eb9))
- _(core)_ Remove store_metadata from FileStoreService - ([bbeaba4](https://github.com/szinn/BookBoss/commit/bbeaba43813656ff308a64cb156e7ca971940d5c))
- _(core)_ Move enrichment handler to core and update PipelineServiceImpl - ([b8bf76e](https://github.com/szinn/BookBoss/commit/b8bf76ec0276df69b0589e506a48df8967820da6))
- _(core)_ Add FormatService port trait and format domain module - ([7ea75e7](https://github.com/szinn/BookBoss/commit/7ea75e72a42114e634872c5d431f806f1b6e2c5b))
- _(core)_ Add before_start() lifecycle hook per crate - ([d411786](https://github.com/szinn/BookBoss/commit/d411786637be6d32207c474b741bc2c7d4018820))
- _(core)_ Handlers take Arc<CoreServices> instead of individual deps - ([9293ad3](https://github.com/szinn/BookBoss/commit/9293ad33ba008c9b34b7257426d472172a3e37fb))
- _(core)_ Expose RepositoryService as pub(crate) on CoreServices - ([3dd17f5](https://github.com/szinn/BookBoss/commit/3dd17f581a319f79edfb6e3313e4c59660eab480))
- _(core)_ Extract HealthService trait from HealthTaskState + HealthTrigger - ([25f52ec](https://github.com/szinn/BookBoss/commit/25f52ec8d90373e58444db22e64904bf0ab9ccb6))
- _(core)_ Absorb JobRegistry into JobService - ([e06bfac](https://github.com/szinn/BookBoss/commit/e06bfac07a1424bfa3a5bc6635e44a096b83c278))
- _(core)_ Rename LibraryStore → FileStoreService, LocalLibraryStore → LocalFileStore - ([1e7c686](https://github.com/szinn/BookBoss/commit/1e7c6867a9c14d4fa4fb25a7b8cc388e3b0eb916))
- _(formats)_ Implement FormatServiceImpl with real logic - ([e37dc35](https://github.com/szinn/BookBoss/commit/e37dc353f59471505109a68e85c2aacc5338dde1))

## [0.7.0](https://github.com/szinn/BookBoss/compare/v0.6.1..v0.7.0) - 2026-03-23

### Features

- _(core)_ Add health task to purge expired sessions daily - ([6f7e883](https://github.com/szinn/BookBoss/commit/6f7e8834f11544277a5bd38d3910fcf52cbf959b))
- _(frontend)_ Make series tile covers link to series detail page - ([4097c92](https://github.com/szinn/BookBoss/commit/4097c9239639f67d315bc37182e877ca5c2918b1))
- _(frontend)_ Add HTTP response compression middleware - ([ebd420e](https://github.com/szinn/BookBoss/commit/ebd420ea3fe223a80edf02fdc639d2114be304dd))

### Bug Fixes

- _(formats)_ Handle XML entity references in OPF text content - ([14339e4](https://github.com/szinn/BookBoss/commit/14339e447658359555d595e3e89e476e7b9a2ae3))
- _(formats)_ Preserve EPUB cover declarations during enrichment - ([04bf4a9](https://github.com/szinn/BookBoss/commit/04bf4a9dbafe276bfff95b2f90ce56a882c6d95f))
- _(frontend)_ Sort series tile covers by series number before limiting - ([c8376fd](https://github.com/szinn/BookBoss/commit/c8376fd68cb0df4cf320353f75ad20ae00024c95))
- _(frontend)_ Close delete-book modal immediately on confirm - ([edba9fb](https://github.com/szinn/BookBoss/commit/edba9fba56ab9a6efb632c62358e016e45e5ed99))
- _(frontend)_ Sort health tasks alphabetically by name - ([662b07c](https://github.com/szinn/BookBoss/commit/662b07c9f790ca67844aa76f75332477e1519d81))

### Refactor

- _(core)_ Remove unused preferred_format from Device model - ([e7f4bdb](https://github.com/szinn/BookBoss/commit/e7f4bdbb0f48e1c3c4768bb7f4bed29e85fafcc5))

### Documentation

- Flesh out installation guide and add Trash documentation - ([d07a83c](https://github.com/szinn/BookBoss/commit/d07a83c9a0e9d21102e1c7a5ed1dce14d2dafdae))

## [0.6.1](https://github.com/szinn/BookBoss/compare/v0.6.0..v0.6.1) - 2026-03-23

### Features

- _(core)_ Add count_books_for_author and count_books_for_series to BookService - ([37ee7dc](https://github.com/szinn/BookBoss/commit/37ee7dc08f4d7bc290ab4697499b7ac66f8b9bfb))
- _(deploy)_ Add example docker-compose files for all database backends - ([1d7e8de](https://github.com/szinn/BookBoss/commit/1d7e8def7f9f4bb5fcfbfda59189f81bff16b733))
- _(frontend)_ Add authors and series listing pages - ([eb7afcf](https://github.com/szinn/BookBoss/commit/eb7afcf7755c58f2f70febbe293ce8c18ba2d654))

### Bug Fixes

- _(frontend)_ Exit selection mode on any route change - ([15e0222](https://github.com/szinn/BookBoss/commit/15e0222873baa4c50b1081153d93a2657782610c))

## [0.6.0](https://github.com/szinn/BookBoss/compare/v0.5.3..v0.6.0) - 2026-03-22

### Features

- _(core)_ SSE event plumbing for system messages - ([a147e65](https://github.com/szinn/BookBoss/commit/a147e656f0e8f35534a395ce37115985b377bbb4))
- _(core)_ Health check subsystem and bookboss wiring - ([03278d1](https://github.com/szinn/BookBoss/commit/03278d1c023cf7f85f513a2cb1d7a0e217e6066f))
- _(core)_ Health check job handlers - ([727255c](https://github.com/szinn/BookBoss/commit/727255c908ec3f390e80ab34b1a858c7fdcd3caf))
- _(core)_ New repository methods for health check tasks - ([4f13844](https://github.com/szinn/BookBoss/commit/4f1384464d751c887951af57b2315d2c898e5659))
- _(core)_ Health task registry and scheduling state - ([bbe3d54](https://github.com/szinn/BookBoss/commit/bbe3d5406bc7172a392f45dad4e88ceea152aede))
- _(core)_ System message domain model, repository, and service - ([e33534d](https://github.com/szinn/BookBoss/commit/e33534d65e93807860fb7c54e7f98d2c0fc0c034))
- _(database)_ Add created_at to book_files for stale enrichment detection - ([37e0bd5](https://github.com/szinn/BookBoss/commit/37e0bd52a443a8792c4df73c8c8eae37b6c6f33b))
- _(frontend)_ Add bulk delete action to multi-select action bar - ([b4968be](https://github.com/szinn/BookBoss/commit/b4968be078c68b01d2c68ea6f193821486a8d5f2))
- _(frontend)_ Navbar badge shows total job queue count, tasks auto-refresh - ([af8a782](https://github.com/szinn/BookBoss/commit/af8a7821b3f438d60bacfef6eb210d5f5f3b9e67))
- _(frontend)_ Persist settings page section across browser refresh - ([6c1d3cf](https://github.com/szinn/BookBoss/commit/6c1d3cffe1d9dbebe9d796bdcd75bf19ecd30188))
- _(frontend)_ Improve timestamps in settings tasks and messages - ([f469ddb](https://github.com/szinn/BookBoss/commit/f469ddbaf5decfe0d2d0ffe209e4c16614b29e81))
- _(frontend)_ Tasks and messages sections in settings page - ([4df0332](https://github.com/szinn/BookBoss/commit/4df0332cb145d4c15e2319596e593c411a9348c0))

### Bug Fixes

- _(core)_ Pass authors to fetch_from_provider for title search scoring - ([42fa4e6](https://github.com/szinn/BookBoss/commit/42fa4e6a42af034c9562f4480d816cb2e426c2e1))
- _(database)_ Filter enrichment recovery queries to Available books only - ([1e36bb3](https://github.com/szinn/BookBoss/commit/1e36bb334fab920eedc83fe541b8cb6732aa848d))
- _(frontend)_ Guard SSE EventSource against duplicate connections - ([c2d11ed](https://github.com/szinn/BookBoss/commit/c2d11ed6613e29cc9186a6cb261e9f6af4da716d))
- _(frontend)_ Trigger_health_task enqueues job directly instead of waiting for poll - ([c85f88b](https://github.com/szinn/BookBoss/commit/c85f88b98668cdf21adb38dc0ac9463897c67c04))
- _(frontend)_ AutocompleteInput commits single keystrokes as values - ([e9a1ff7](https://github.com/szinn/BookBoss/commit/e9a1ff7e67591d18024b0b411c4fda512df047b6))

### Documentation

- Update README and mdBook docs to reflect current project state - ([ecfde4d](https://github.com/szinn/BookBoss/commit/ecfde4d164d7cfeb395447103d8a85e871529840))

### Stying

- _(frontend)_ Center-align task list columns - ([b2963a9](https://github.com/szinn/BookBoss/commit/b2963a94b0aa7d50b83d104de49f650bbe38681c))

### Miscellaneous Tasks

- _(core)_ Polish health check wiring and remove dead code - ([764092d](https://github.com/szinn/BookBoss/commit/764092dad0a361360db4ce68c79f8ed0e05e97df))

## [0.5.3](https://github.com/szinn/BookBoss/compare/v0.5.2..v0.5.3) - 2026-03-22

### Features

- _(core)_ SSE real-time event system with EventService abstraction - ([ad21cea](https://github.com/szinn/BookBoss/commit/ad21cea8581ef62f5b9317dd8bd36fb2477f9050))
- _(frontend)_ Phase 4 polish — keyboard shortcuts, permission gating, route-aware selection cleanup - ([9471147](https://github.com/szinn/BookBoss/commit/9471147ee3fa316bbe0be440a20b032d8b7423e9))
- _(frontend)_ Bulk edit metadata modal for selected books - ([5343ce5](https://github.com/szinn/BookBoss/commit/5343ce5f702a64caec0a7f6f5f2e5b02b211a4a7))
- _(frontend)_ Bulk set reading status for selected books - ([fc604f5](https://github.com/szinn/BookBoss/commit/fc604f5836500e7f2d8862ff1b5ec150a8524c82))
- _(frontend)_ Add multiselect infrastructure for bulk book operations - ([86db3f5](https://github.com/szinn/BookBoss/commit/86db3f577878f7e4e5e452999a184f53fd25c4b1))

### Bug Fixes

- _(frontend)_ Show reading status and progress on shelf book cards - ([7f9d40a](https://github.com/szinn/BookBoss/commit/7f9d40a7fef165ea394132437304d451ab87ba00))

## [0.5.2](https://github.com/szinn/BookBoss/compare/v0.5.1..v0.5.2) - 2026-03-22

### Features

- _(core)_ Cover selection from confident providers only - ([f6c9cd7](https://github.com/szinn/BookBoss/commit/f6c9cd759fa411337e7fff86d8961f8a7e7e3437))
- _(core)_ Parallel provider enrichment with title+author similarity scoring - ([8569422](https://github.com/szinn/BookBoss/commit/85694220b650f618968ae38f3bce6c18c2afd189))
- _(metadata)_ Open Library title search fallback - ([7c4d9d5](https://github.com/szinn/BookBoss/commit/7c4d9d5cd50660f23e2758d55a4ab9f3e865cc7c))
- _(metadata)_ Google Books title search fallback + provider priority tiebreaker - ([d312246](https://github.com/szinn/BookBoss/commit/d31224653b81e7105e6f62e6cbdb8245557dc67c))
- _(metadata)_ Hardcover title search fallback - ([fdf0021](https://github.com/szinn/BookBoss/commit/fdf0021a9a2489204d4e86f888cc9581b599566d))

### Bug Fixes

- _(database)_ Fix integration tests broken by available-only list_books filter - ([88f87bc](https://github.com/szinn/BookBoss/commit/88f87bc0b8b862fb46bf6d90b41982a0e8e93fd2))
- _(frontend)_ Avoid unnecessary ownership in kobo init response builder - ([59540ac](https://github.com/szinn/BookBoss/commit/59540acc0f839f55ab4aa9e4fc4ccb12cb20f014))

### Refactor

- _(core,database,frontend)_ Pass token types by value instead of reference - ([9c75190](https://github.com/szinn/BookBoss/commit/9c75190cb038e9fefd5053b1407c6874a51388ff))

## [0.5.1](https://github.com/szinn/BookBoss/compare/v0.5.0..v0.5.1) - 2026-03-21

### Features

- _(core,database,frontend)_ Add sortable book display with sort control UI - ([52f1956](https://github.com/szinn/BookBoss/commit/52f1956360fb9ead55c676859454644274604c2b))
- _(frontend)_ Add search bar to NavBar for live client-side book filtering - ([8852f48](https://github.com/szinn/BookBoss/commit/8852f48fb97268779bec81d234a295b1107f0a85))
- _(frontend)_ Dismiss all modals on Escape key - ([c56da4a](https://github.com/szinn/BookBoss/commit/c56da4aa3739f014af816b1cf6e745fc15c76e9e))
- _(import)_ Disconnect bookdrop scanner from timer; add UI refresh button - ([bf2cf5b](https://github.com/szinn/BookBoss/commit/bf2cf5b9d504cd591fdce8358a7969bf328703a7))

### Bug Fixes

- _(database,frontend)_ Pass trivially-copy op types by value - ([a0d2dc9](https://github.com/szinn/BookBoss/commit/a0d2dc97b4f483eec0f4ce19c09ca8b9bafbcf37))
- _(frontend)_ Increase spacing before recheck button in Incoming nav item - ([eda5543](https://github.com/szinn/BookBoss/commit/eda554336286b2183f0022c3ce82af5d4ef5f290))
- _(frontend)_ Replace map().unwrap_or() with map_or() - ([2d3e813](https://github.com/szinn/BookBoss/commit/2d3e813ab8cc32c7475df09453c0443bd80b3093))
- _(storage,frontend)_ Rewrite match-to-return as let...else - ([68d6989](https://github.com/szinn/BookBoss/commit/68d6989d6401652658c8bd86f806a3faf8fc600c))

### Refactor

- _(core)_ Replace Nop mocks with mockall in test_support - ([beb2c3f](https://github.com/szinn/BookBoss/commit/beb2c3f68712cc206aee407dd3fb989ce4f8984a))
- _(core)_ Introduce JobService port for background job enqueueing - ([6f35a68](https://github.com/szinn/BookBoss/commit/6f35a6854d553d79d05f454f58d0c4cdc5c58405))
- _(core)_ Introduce ExternalServices builder for create_services - ([9fe82bb](https://github.com/szinn/BookBoss/commit/9fe82bbf17d9482e174ac7c48bd327b96c3bcac5))
- _(core,database,frontend)_ Remove status from BookQuery; enforce available in adapter - ([7358ace](https://github.com/szinn/BookBoss/commit/7358ace5c4b6b1ab85b244e71aa6204e35e6fd9c))
- _(frontend)_ Display OPDS username and password on the same line - ([dd384dc](https://github.com/szinn/BookBoss/commit/dd384dc71479f3dde37ecc5f3cd9f5261d12696c))
- _(import)_ Remove RepositoryService from bb-import adapters - ([0f132a0](https://github.com/szinn/BookBoss/commit/0f132a0db752a9d20b7ff23ce6e872464a1fa017))

### Miscellaneous Tasks

- _(build)_ Opt-level=1 for all workspace crates in dev profile - ([7af6d83](https://github.com/szinn/BookBoss/commit/7af6d8356e0515ee7e96bbbd5e157b0921e7149b))

## [0.5.0](https://github.com/szinn/BookBoss/compare/v0.4.6..v0.5.0) - 2026-03-20

### Features

- _(api)_ Add OPDS OpenSearch endpoint (M13.6) - ([9008733](https://github.com/szinn/BookBoss/commit/900873347a6a6c7b80fb9266ecf395aa19ab9b80))
- _(frontend)_ Redesign DeviceCard layout - ([89359cb](https://github.com/szinn/BookBoss/commit/89359cb24f5f9e360bd57686b7db7fb7a4aa47ad))

### Bug Fixes

- _(frontend)_ Use stable key for chip_input spans - ([5cec235](https://github.com/szinn/BookBoss/commit/5cec2356dbcfbb5fb9880e14924488174a567952))
- _(frontend)_ Use FrontendConfig as an extension - ([3a51c7f](https://github.com/szinn/BookBoss/commit/3a51c7f3873be4cce306ebfd9527f171ef89ea83))

### Refactor

- _(core)_ Consolidate test mock infrastructure - ([40e2e37](https://github.com/szinn/BookBoss/commit/40e2e37646094ffaa46915ff62716957e34896d2))
- _(core)_ Introduce LibraryRepository for catalog-level queries - ([df7c970](https://github.com/szinn/BookBoss/commit/df7c970a40725aa3167655aa0b8885d75712a0e2))
- _(core,frontend)_ Remove auto-read threshold and Reading settings - ([ee45fef](https://github.com/szinn/BookBoss/commit/ee45fef2dea7267cfc474b478d82ce140abc5b16))
- _(frontend)_ Move OPDS regenerate button inline with password - ([8bbc164](https://github.com/szinn/BookBoss/commit/8bbc164048f141285fcc061f1f2830e217bd69b4))
- _(frontend)_ Move device list above OPDS section - ([ae537d4](https://github.com/szinn/BookBoss/commit/ae537d400390cff3dcba2e119e826e4eed7f7c3a))

## [0.4.6](https://github.com/szinn/BookBoss/compare/v0.4.4..v0.4.6) - 2026-03-19

### Features

- _(core)_ Replace Argon2 hashing with AES-256-GCM encryption for OPDS passwords - ([403ac17](https://github.com/szinn/BookBoss/commit/403ac174ab86eb12823d233de2e42350dcb9fdf3))
- _(core)_ Add OpdsAccess capability and OPDS password service - ([c2829fd](https://github.com/szinn/BookBoss/commit/c2829fdb32bfd76b425bb6eb7f0e353ee299afe2))
- _(frontend)_ Add OPDS section to profile page - ([d74da75](https://github.com/szinn/BookBoss/commit/d74da75e32e717cacb1c9a7fdb7fdd71989d5224))
- _(frontend)_ Add OPDS download and cover endpoints - ([afc3fcc](https://github.com/szinn/BookBoss/commit/afc3fcc0de1b2939605071a3ca473843a8b19ebb))
- _(frontend)_ Add OPDS author and series navigation and acquisition feeds - ([7f11312](https://github.com/szinn/BookBoss/commit/7f113123104d8ba3ad816671acf07a9f54c55c56))
- _(frontend)_ Add OPDS shelf acquisition feed - ([5aca37d](https://github.com/szinn/BookBoss/commit/5aca37d4ef718b1b7f2ba6095c0e65513eb56fbc))
- _(frontend)_ Add OPDS root catalog, all books, and shelves feeds - ([62787b3](https://github.com/szinn/BookBoss/commit/62787b31a51d9887f2d83cd21968a841b73b34db))
- _(frontend)_ Add OPDS module with Basic Auth extractor and Atom XML helpers - ([b33f736](https://github.com/szinn/BookBoss/commit/b33f73621ee92df12fad0971d45ead461c18cd93))
- _(frontend)_ Make author and series names clickable links in book cards - ([41cdc46](https://github.com/szinn/BookBoss/commit/41cdc46280c9a5a03804031b921259b909477dae))

### Bug Fixes

- _(core)_ Fix OpdsService::get_or_create_password and add service tests - ([d61a124](https://github.com/szinn/BookBoss/commit/d61a124892c015d22396fa7442d65dbee8e002c1))
- _(core)_ Use trait parameter names in mock AuthorRepository/ImportJobRepository impls - ([ebade68](https://github.com/szinn/BookBoss/commit/ebade682e722349cfd51609da55627b72a68691e))
- _(database)_ Use Func::cust for REPLACE to fix Postgres compatibility - ([01a135a](https://github.com/szinn/BookBoss/commit/01a135a222250dc02aac1fa1a4602582ba925d30))
- _(frontend)_ Add /opds route without trailing slash - ([5ccb544](https://github.com/szinn/BookBoss/commit/5ccb54405c2c03ea1fa25e540528ed9276ba6dff))
- _(frontend)_ Use full base_url for OPDS catalog URL on profile page - ([9722808](https://github.com/szinn/BookBoss/commit/9722808a8c260a390f9953b50fc3fdadc0cf4ff7))

### Refactor

- _(core)_ Replace hand-rolled Mock* structs with mockall - ([2070198](https://github.com/szinn/BookBoss/commit/207019886270c0770be940972cb538d49503cf62))
- _(core)_ Move string conversions onto enum types - ([5a8fb25](https://github.com/szinn/BookBoss/commit/5a8fb25357c5c69ef5a644bacd2a35e3f7be2d21))
- _(core,database,frontend,formats,storage)_ Move string conversions onto types - ([2d0ca79](https://github.com/szinn/BookBoss/commit/2d0ca79bc143f0391a384e4dfaaf818279fe8c29))
- _(frontend)_ Extract shared server function helpers - ([cdee53a](https://github.com/szinn/BookBoss/commit/cdee53a15faf85da4bdcb682e95db955ee9b8d17))

### Documentation

- _(readme)_ Update feature list - ([678ffd2](https://github.com/szinn/BookBoss/commit/678ffd281f75a9615a49756e11b3b74915d90b76))

## [0.4.4](https://github.com/szinn/BookBoss/compare/v0.4.3..v0.4.4) - 2026-03-16

### Features

- _(core)_ Add Kobo reading state fields to UserBookMetadata - ([acb6020](https://github.com/szinn/BookBoss/commit/acb602071465dd0b21d8f4945c855c05343f6710))
- _(database)_ Update SeaORM entity and adapter for Kobo reading state fields - ([e57ccd3](https://github.com/szinn/BookBoss/commit/e57ccd3486901c64b15fded8ed631351a7cb00af))
- _(database)_ Add position_type, spent/remaining_reading_minutes columns to user_book_metadata - ([5465307](https://github.com/szinn/BookBoss/commit/5465307d9eeb7e108bd246b3bf05346024f0d071))
- _(formats)_ Wire ConvertKepubHandler into server startup - ([ccff46e](https://github.com/szinn/BookBoss/commit/ccff46ea798a99e2051cc5bd2c4f555f0fb49daa))
- _(formats)_ Chain EnrichEpubHandler → ConvertKepubJob - ([3480466](https://github.com/szinn/BookBoss/commit/34804660b6678422611befab0aa8dcf280058848))
- _(formats)_ Add ConvertKepubHandler and recovery - ([3842c9b](https://github.com/szinn/BookBoss/commit/3842c9b60018b145212c210dab39e61dfa027b21))
- _(formats)_ Implement in-house EPUB→KEPUB conversion - ([ccbcb00](https://github.com/szinn/BookBoss/commit/ccbcb00540ae2cdcf114177feea102627116ea50))
- _(formats)_ Add ConvertKepubPayload and queue_convert_kepub to ConversionService - ([75be0d9](https://github.com/szinn/BookBoss/commit/75be0d9b7447c66203522704e2282cc51ab8ed27))
- _(frontend)_ Embed ReadingState in Kobo library sync responses - ([3d918bf](https://github.com/szinn/BookBoss/commit/3d918bfefd10af88d6fb5ccb3026c6ded1d8031c))
- _(frontend)_ Implement Kobo reading state GET and PUT endpoints - ([45b5d37](https://github.com/szinn/BookBoss/commit/45b5d37e4f324f8d5b064ed004a47e7e1e012908))

### Bug Fixes

- _(formats)_ Correct KEPUB filename — strip .epub not -enriched.epub - ([e4d5a33](https://github.com/szinn/BookBoss/commit/e4d5a333b188c89143b97923be04b8e8dfb9daf9))
- _(frontend)_ Kobo state PUT/GET and library sync correctness - ([8814336](https://github.com/szinn/BookBoss/commit/8814336e73770664a3c4806974e7ada6eb738a24))
- _(frontend)_ Use fallback resources when Kobo store init returns non-Resources JSON - ([88be1eb](https://github.com/szinn/BookBoss/commit/88be1ebd8218fd96fde8e7692d7962d707bef0ea))
- _(frontend)_ Accept single JSON object for Kobo state PUT body - ([f709b26](https://github.com/szinn/BookBoss/commit/f709b26046ff8ecce043f5a34fa510a60c0c185a))
- _(frontend)_ Accept single JSON object for Kobo state PUT body - ([e2b1d48](https://github.com/szinn/BookBoss/commit/e2b1d4839c18c1d230f8e2d16d74b38f8bbd9bd3))
- _(frontend)_ Resolve book once at top of Kobo state PUT handler - ([6c05de5](https://github.com/szinn/BookBoss/commit/6c05de570985f43c93a3271720221798169b79d9))

### Miscellaneous Tasks

- Update scratchpad — M11 and M12 complete - ([765681e](https://github.com/szinn/BookBoss/commit/765681eb3a159461438caa04eb9ad1c7ba32ccf1))

## [0.4.3](https://github.com/szinn/BookBoss/compare/v0.4.2..v0.4.3) - 2026-03-16

### Features

- _(frontend)_ Series field pill display with new badge + focus dropdown - ([eb7e35c](https://github.com/szinn/BookBoss/commit/eb7e35c125929f7361e2ea2d706011fa94d136c7))
- _(frontend)_ ChipInput shows dropdown on focus when empty - ([7f2924c](https://github.com/szinn/BookBoss/commit/7f2924c8ac0050313d311de62a9ce0721acad4f8))
- _(frontend)_ Publisher pill input with autocomplete + new badge - ([67f2caa](https://github.com/szinn/BookBoss/commit/67f2caa2e944bddab7a6d52598de443d29df59a2))
- _(frontend)_ About modal on logo + admin-only settings icon - ([5974c6e](https://github.com/szinn/BookBoss/commit/5974c6eba5b06c17c4500f2ced43fbb976d8b7e4))

### Bug Fixes

- _(frontend)_ ChipInput re-shows dropdown when input is cleared - ([f571191](https://github.com/szinn/BookBoss/commit/f571191eca84b91e1a60fd913372e59c7b8a4d7c))
- _(frontend)_ Kobo auth + transparent store proxy - ([fc29ef3](https://github.com/szinn/BookBoss/commit/fc29ef353b4cb6a32014fbeb9e38ea4c5c9908f6))

## [0.4.2](https://github.com/szinn/BookBoss/compare/v0.4.1..v0.4.2) - 2026-03-15

### Bug Fixes

- _(frontend)_ Kobo auth — use Kobo store for device_auth/refresh, add init header - ([1f1e20b](https://github.com/szinn/BookBoss/commit/1f1e20b92d7ce42b280b5dfd329f61adc198daae))
- _(frontend)_ Fix Kobo auth flow — correct resource keys, response format, and init header - ([33d1da6](https://github.com/szinn/BookBoss/commit/33d1da66dd0d523d0a3d2144ed978892c3b8e526))
- _(frontend)_ Return real sync token as UserKey + add auth/device refresh endpoint - ([2795328](https://github.com/szinn/BookBoss/commit/27953280790cc801d418d961f150e1bc96788b89))

## [0.4.0](https://github.com/szinn/BookBoss/compare/v0.3.4..v0.4.0) - 2026-03-15

### Features

- _(core)_ Add find_device_by_token to DeviceService (M8.3.1) - ([1ba8b4f](https://github.com/szinn/BookBoss/commit/1ba8b4fbc7f56129303b382597083a41184a3186))
- _(core)_ Implement apply_sync and compute_sync_diff tests (M8.2.5, M8.2.6) - ([2d63a93](https://github.com/szinn/BookBoss/commit/2d63a935738b9443210b90496a4adc023580890f))
- _(core)_ Implement DeviceServiceImpl::compute_sync_diff (M8.2.4) - ([39279e7](https://github.com/szinn/BookBoss/commit/39279e7c96846eb680c2c8a34cb399cb89af280a))
- _(core)_ M8.2.3 — add compute_sync_diff and apply_sync to DeviceService trait - ([8b5d5a2](https://github.com/szinn/BookBoss/commit/8b5d5a28550c9e4abc0101b904c4c9025974d770))
- _(core)_ M8.2.2 — add SyncDiff and BookSyncEntry types - ([bcb305c](https://github.com/szinn/BookBoss/commit/bcb305cae081b130107b4adf9f7ae102a06678af))
- _(database)_ M8.2.1 — replace device_books.book_file_id with file_role - ([35de3bd](https://github.com/szinn/BookBoss/commit/35de3bd86adfe44a340ac4dbfe6aabdfc8b5f0c2))
- _(database)_ M8.1 — rework device_books schema for kobo sync - ([72bda7e](https://github.com/szinn/BookBoss/commit/72bda7ee10288105461bad27a0d25d4d51aea6d5))
- _(frontend)_ Handle Kobo DELETE library and no-cache cover images - ([585efec](https://github.com/szinn/BookBoss/commit/585efec4acc9548f996195e772e2b743e53eae20))
- _(frontend)_ M8.9 — device card last_synced_at and sync URL copy button - ([2a0495f](https://github.com/szinn/BookBoss/commit/2a0495f6b358ecbf34a8e881ff9393a91887f651))
- _(frontend)_ M8.8 — Kobo ancillary endpoints and catch-all - ([82b1994](https://github.com/szinn/BookBoss/commit/82b19941deb46cf9d2d6d801076634759e21ba7c))
- _(frontend)_ M8.7 — Kobo cover image endpoint - ([73870b9](https://github.com/szinn/BookBoss/commit/73870b99e70bf32f60acb8d035644e717e2830f0))
- _(frontend)_ Implement Kobo book file download endpoint (M8.6) - ([4b2c9d5](https://github.com/szinn/BookBoss/commit/4b2c9d59ea5dcf7e44aa588b2cc26c8f9e8f8628))
- _(frontend)_ Implement Kobo library sync endpoint (M8.5) - ([e5b9831](https://github.com/szinn/BookBoss/commit/e5b9831917b777a9f6d070a7da7269ab9d9de446))
- _(frontend)_ Implement POST /kobo/{sync_token}/v1/initialization (M8.4) - ([75f2838](https://github.com/szinn/BookBoss/commit/75f2838fd248355d196cc2060e3a5c55dc12052d))
- _(frontend)_ Add Kobo sync router, extractor, and cursor (M8.3.2-M8.3.4) - ([d52c5ea](https://github.com/szinn/BookBoss/commit/d52c5eae2ed159b3ff27a726f92edc97d8477f04))
- _(frontend)_ Added base url for application - ([730e832](https://github.com/szinn/BookBoss/commit/730e8322dbf8f4acd3f0584172f0feeed8bf2da2))
- _(storage)_ Resize and re-encode JPEG covers on import - ([04677bc](https://github.com/szinn/BookBoss/commit/04677bc6f475b813cc2d671175cf8ac9a7d99ff2))
- _(storage)_ Strip EXIF from JPEG covers on import - ([ab1b025](https://github.com/szinn/BookBoss/commit/ab1b025f12e05a251e0801e150994b257158123d))

### Bug Fixes

- _(core,frontend)_ Use last_synced_at=None as full-sync override - ([99d56c9](https://github.com/szinn/BookBoss/commit/99d56c9f68517a81534cc30b5034c89aff74a765))
- _(core,frontend,bookboss)_ Kobo delete entitlement + on-removal action + tracing - ([bc71562](https://github.com/szinn/BookBoss/commit/bc71562b27981923fb830e592b6856da32b7c5b3))
- _(frontend)_ Hide delete modal synchronously on confirm - ([ce99ef6](https://github.com/szinn/BookBoss/commit/ce99ef6d6dbc471625b722a3aa94152d9bcaf13a))
- _(frontend)_ Send current time as bookmark_date when last_synced_at is None - ([2aa43c6](https://github.com/szinn/BookBoss/commit/2aa43c6b1d7ad5cb41e23aa22f60e0a88b3e9fd5))
- _(frontend)_ Replace reset button with last-synced-as-button - ([0ea7569](https://github.com/szinn/BookBoss/commit/0ea7569bd22dfc38682ad75eec2170714cf74cdd))
- _(frontend)_ Fix clipboard copy and checkmark centering in device card - ([65217c0](https://github.com/szinn/BookBoss/commit/65217c0d4776782435cbc1df64c6593f07d07ccc))
- _(frontend)_ Add x-kobo-apitoken header and GET support to initialization endpoint - ([5cb8fdf](https://github.com/szinn/BookBoss/commit/5cb8fdf10f113d56f5f92464b200d5d1554752e6))
- _(frontend)_ Fix copy button hang in DeviceCard - ([71eebea](https://github.com/szinn/BookBoss/commit/71eebea4b385bb2ddbabe0d943338dfe12de99e7))

### Refactor

- _(core)_ M9.3 - LibraryStore trait: resolve() + M9.7 download handlers - ([f31d345](https://github.com/szinn/BookBoss/commit/f31d345f6b01599ee59aaae894c3801c579c156a))
- _(core,database)_ Replace original_filename with path on BookFile - ([ad0053a](https://github.com/szinn/BookBoss/commit/ad0053af0253a4fcefbf2f7b84fc25cabb115261))
- _(core,formats)_ M9.4/M9.5/M9.6 - wire BookFile.path through pipeline - ([53d7b04](https://github.com/szinn/BookBoss/commit/53d7b04bf652b799d03d9039890b994a3ebe6308))
- _(database)_ Merge migrations 23+24 into create_device_books_table - ([fd3db3e](https://github.com/szinn/BookBoss/commit/fd3db3ec8f295b3c02b9abc0120e4f39e7c85aad))

### Testing

- _(core)_ Add component tests for JobRegistry and LibraryService - ([b3fd552](https://github.com/szinn/BookBoss/commit/b3fd552c66f0409bd3a8f03c261ec711809471ff))
- _(integration)_ Add reading state, device integration tests and device DB adapter component tests - ([571e72e](https://github.com/szinn/BookBoss/commit/571e72e3c0b85fef560493a9e0e241433d1424c8))
- _(integration)_ Add pipeline and shelf integration tests - ([141245c](https://github.com/szinn/BookBoss/commit/141245cb202074db8e2aee7b6b14051568b38257))

### Miscellaneous Tasks

- _(database)_ Replace original_filename with path in book_files schema - ([7c774c6](https://github.com/szinn/BookBoss/commit/7c774c6a2959a3a8f12959ef4afff2c9f708e9ce))

## [0.3.3](https://github.com/szinn/BookBoss/compare/v0.3.2..v0.3.3) - 2026-03-15

### Features

- _(core)_ Trigger EPUB enrichment after approve_job and edit_book (M7.8) - ([f3b0763](https://github.com/szinn/BookBoss/commit/f3b07634cd8625bbab1d8c61498d249f22ff70f8))
- _(core)_ M7.1 — add file_role and Kepub to book file model - ([1bdf59a](https://github.com/szinn/BookBoss/commit/1bdf59a34e20523193930e4e48f76dcf3a54f7e0))
- _(formats)_ Write calibre:series/series_index meta elements; read series from spinnaker blob - ([c063f59](https://github.com/szinn/BookBoss/commit/c063f59a0d674726b94c82ff9edb6bac81fb1c03))
- _(formats)_ Skip metadata providers when spinnaker:metadata is present in EPUB - ([4479705](https://github.com/szinn/BookBoss/commit/447970566586c87677df7e411805b631e07f8cca))
- _(formats)_ M7.7 EnrichEpubHandler + book_slug centralisation - ([71b53cc](https://github.com/szinn/BookBoss/commit/71b53cc8c48071a038a39c66774766e4dbb5cdc4))
- _(formats)_ M7.6 ConversionService port + EnrichEpubPayload - ([6a4a2ae](https://github.com/szinn/BookBoss/commit/6a4a2ae28eff0b6893cb9d84ab14d77887f038fd))
- _(formats)_ Implement M7.5 EPUB enrichment - ([f940cf9](https://github.com/szinn/BookBoss/commit/f940cf95fadeb502bca6787a0ca328eebc4973a9))
- _(frontend)_ ConversionBadge in NavBar showing pending enrichment job count (M7.10) - ([5d0f68e](https://github.com/szinn/BookBoss/commit/5d0f68e999b66ee0f14b01385f1a4004a9e589c9))
- _(frontend)_ Serve enriched files with original fallback, deduplicate format badges (M7.9) - ([2aa2de6](https://github.com/szinn/BookBoss/commit/2aa2de62b80b0ded40ece5269e1783ca46eaf365))
- _(storage)_ Implement M7.2 Originals/ directory layout - ([c707d3a](https://github.com/szinn/BookBoss/commit/c707d3a04fe59484a0e03bd51edcad469e0b5fd5))

### Bug Fixes

- _(core)_ Delete original file when rejecting an import job - ([87bdf10](https://github.com/szinn/BookBoss/commit/87bdf101d671a78f2525bedc1bc4342cb28de1c5))
- _(core)_ Remove rating from spinnaker blob; flow page_count through pipeline - ([c4d5c60](https://github.com/szinn/BookBoss/commit/c4d5c60e6e45ab7df094d3e1f669f3ebbdde091a))
- _(core)_ Delete original file from Originals/ when a book is deleted - ([bcf82d8](https://github.com/szinn/BookBoss/commit/bcf82d8c7ef910f4a83ab6570ec722bb55699e03))
- _(formats)_ Extract genres/tags/publisher/language from EPUB on import - ([70f3566](https://github.com/szinn/BookBoss/commit/70f3566f641f40f7882afadd5c04fcb50494fec9))
- _(formats)_ Parse dc:subject elements as genres; print genres in dump-epub - ([780f3a0](https://github.com/szinn/BookBoss/commit/780f3a0b0151cd9bacfccc19bb22085c9686b9bd))
- _(frontend)_ Fix stale cover images not refreshing after update - ([659a179](https://github.com/szinn/BookBoss/commit/659a179fdb9a1396affe85cba9ac4bc90321b054))

### Testing

- _(formats)_ Add dc:subject and full-field roundtrip tests for OPF and EPUB enrich - ([a42d5fd](https://github.com/szinn/BookBoss/commit/a42d5fd8fcc1ffb0ec0d33c15a29de829a9fff8e))

## [0.3.2](https://github.com/szinn/BookBoss/compare/v0.3.1..v0.3.2) - 2026-03-14

### Bug Fixes

- _(frontend)_ Fix Incoming nav link not rendering after page refresh - ([ece7f35](https://github.com/szinn/BookBoss/commit/ece7f3552d737ae8f9a68f7b8bd818f5c1745052))

### Performance

- _(database)_ Add missing indices to existing migrations - ([656563e](https://github.com/szinn/BookBoss/commit/656563edcb5729efdc26594257a88d4e1ff00be0))

## [0.3.0](https://github.com/szinn/BookBoss/compare/v0.2.5..v0.3.0) - 2026-03-14

### Features

- _(core)_ MP.4 + MP.5 — DeviceService trait, impl, and CoreServices wiring - ([e8de684](https://github.com/szinn/BookBoss/commit/e8de68438588ed3ebe19caad8db07ac5db66749e))
- _(core)_ MP.3 — ShelfService::create_device_shelf + find_device_shelf - ([021c5aa](https://github.com/szinn/BookBoss/commit/021c5aae3e34be47cc5c569746ee3784f8fad69d))
- _(core)_ Add filter domain with BookFilter tree model - ([23422e1](https://github.com/szinn/BookBoss/commit/23422e1dddb02c50810e62b4ed9a703b2c6e8de4))
- _(core,database)_ MP.2 — ShelfRepository::find_by_device_id - ([db12232](https://github.com/szinn/BookBoss/commit/db1223247ae9c8c41ff9c970e7dc3f83be3566cb))
- _(core,database)_ MP.1 — DeviceRepository CRUD + database adapter - ([1e9a4bb](https://github.com/szinn/BookBoss/commit/1e9a4bb74f65320d9b0a324a0d16cbaf748fbf61))
- _(core,database)_ Replace ShelfFilter with BookFilter tree (M6.2) - ([f35764f](https://github.com/szinn/BookBoss/commit/f35764f1ec5b7e835d24706bb0657ff810c43c05))
- _(database)_ Implement build_condition() filter translator (M6.3) - ([e40e060](https://github.com/szinn/BookBoss/commit/e40e060a4f2f63f76712b3fc2cc43c3882eba087))
- _(frontend)_ MP.13 — disable shelf delete for device-linked shelves - ([df254b9](https://github.com/szinn/BookBoss/commit/df254b96666a62aca58cd1fe410fb6e8ea404a6a))
- _(frontend)_ MP.12 — My Devices section on Profile page - ([97e3a6f](https://github.com/szinn/BookBoss/commit/97e3a6fe21e262397ee4c44757a2c59d9b0fb961))
- _(frontend)_ MP.11 — move Reading section from Settings to Profile - ([7942131](https://github.com/szinn/BookBoss/commit/79421312bbfa3b16d075247da760da2663bc02ad))
- _(frontend)_ MP.10 — profile section with modal password change - ([7e53ce2](https://github.com/szinn/BookBoss/commit/7e53ce20ff7503c04a87c79aab06da73fa32cc08))
- _(frontend)_ Rework ProfilePage to single-page scroll layout - ([23c31d8](https://github.com/szinn/BookBoss/commit/23c31d83f84be5bc76ee70dfe9d1d70f3f2cca72))
- _(frontend)_ MP.8 + MP.9 — Profile link in NavBar and ProfilePage skeleton - ([a5154b2](https://github.com/szinn/BookBoss/commit/a5154b23ed1bb4bc9e4565b1636ad205105a6349))
- _(frontend)_ M6.7 — badge counts on smart shelf pills - ([dabde74](https://github.com/szinn/BookBoss/commit/dabde74c5e8f33cab3854125878f05405ac5c79a))
- _(frontend)_ ReadStatus filter rule — chip input with centered hint - ([fb8abad](https://github.com/szinn/BookBoss/commit/fb8abad464884f5bca437a31c0761b86af1a0149))
- _(frontend)_ M6.6 — smart shelf detail page - ([26f818c](https://github.com/szinn/BookBoss/commit/26f818cf75135c4dd70fd8e797dc6945ce4d7579))
- _(frontend)_ M6.5 smart shelf create/edit UI - ([a1f1547](https://github.com/szinn/BookBoss/commit/a1f15478a6a0d9b6b602d2208fdc530c77c8ba0e))
- _(frontend)_ M6.4 filter builder component and entity options server fn - ([bcb6072](https://github.com/szinn/BookBoss/commit/bcb60724e9cc471d255f27555709148eaec4377f))

### Bug Fixes

- _(core)_ Sync device name when companion shelf is renamed - ([3f406a1](https://github.com/szinn/BookBoss/commit/3f406a10c4f899c7044c094013b83e9b5baa841f))
- _(frontend)_ Resize graphic assets - ([ee46937](https://github.com/szinn/BookBoss/commit/ee46937267448c56c7c14af407a060f94f295493))

### Refactor

- _(core)_ Clippy warning cleanup and lint tuning - ([242a3c7](https://github.com/szinn/BookBoss/commit/242a3c73df0d17f639be1bbdc8a45321f324abad))
- _(core,database,frontend)_ Rename BookFilter → BookQuery in book domain - ([848feb6](https://github.com/szinn/BookBoss/commit/848feb6247d3d371d8e14e0812ee20fdf283d73f))

### Miscellaneous Tasks

- _(bookboss)_ Add mimalloc as global allocator - ([a9452d3](https://github.com/szinn/BookBoss/commit/a9452d3f549e58122f1ae8114beb87ef1d895ef8))
- _(core)_ Suppress remaining cast warnings with #[expect] and reason - ([fae77bf](https://github.com/szinn/BookBoss/commit/fae77bf87ab90930bd7e974eeac34414c03247b4))
- _(core)_ Remove clone_on_ref_ptr lint; allow SeaORM cast warnings - ([eaf8265](https://github.com/szinn/BookBoss/commit/eaf8265f732cc5689bcb7d53d5c15c3db885dbe8))

## [0.2.5](https://github.com/szinn/BookBoss/compare/v0.2.4..v0.2.5) - 2026-03-11

### Features

- _(frontend)_ Keep NavBar stable during route navigation - ([a4e8658](https://github.com/szinn/BookBoss/commit/a4e8658bcb0f428a07f50f430740ddc926a7415b))
- _(frontend)_ Add /healthz and /readyz health endpoints - ([a22c7a2](https://github.com/szinn/BookBoss/commit/a22c7a2309732377c93535eac53ab21fc591409c))

### Bug Fixes

- _(frontend)_ Limit inbound HTTP request body size to 10 MiB - ([1db685f](https://github.com/szinn/BookBoss/commit/1db685f0a0f69b5fcbb05ba99ba5e3f0e60021a1))
- _(frontend)_ Fix banner image width on mobile screens - ([6272ca2](https://github.com/szinn/BookBoss/commit/6272ca2be93455ffbeb5ab046305e47b83b63565))
- _(metadata)_ Add 5s request timeout to all metadata providers - ([a90ada8](https://github.com/szinn/BookBoss/commit/a90ada8c55384245f760caa46b43010abf927b21))

### Miscellaneous Tasks

- _(clippy)_ Clean up some clippy warnings - ([3aa0b3f](https://github.com/szinn/BookBoss/commit/3aa0b3f2b7015275593da793d25ed57aa61105a3))

## [0.2.4](https://github.com/szinn/BookBoss/compare/v0.2.3..v0.2.4) - 2026-03-11

### Features

- _(frontend)_ Display genres and tags on book detail page - ([b598ddf](https://github.com/szinn/BookBoss/commit/b598ddf71add84a139820280ca34b91aae79cb0d))

### Bug Fixes

- _(frontend)_ Tie banner graphic to login/register widths - ([8bbf01a](https://github.com/szinn/BookBoss/commit/8bbf01ab9fa2f1f9defe5f1ae64b33e1c12686fd))
- _(frontend)_ Capability-gate edit/delete buttons and add delete confirmation - ([d22d8b5](https://github.com/szinn/BookBoss/commit/d22d8b5cf14c048bcb69631e7627fed43f256f4a))
- _(frontend)_ Programmatically focus username field on login form mount - ([2171e38](https://github.com/szinn/BookBoss/commit/2171e3834bb746e6241b8cea9f52278ab971c6eb))

### Refactor

- _(frontend)_ Integrate frontend as tokio-graceful-shutdown subsystem - ([8487aec](https://github.com/szinn/BookBoss/commit/8487aec95a86972073bad8df27d1f24670073db3))

## [0.2.3](https://github.com/szinn/BookBoss/compare/v0.2.2..v0.2.3) - 2026-03-09

### Features

- _(core,database,frontend)_ Add full_name and change_password_on_login to user model - ([a7f1bdf](https://github.com/szinn/BookBoss/commit/a7f1bdfa0cbc8d975a2362dda453467601abc0d1))
- _(core,frontend)_ Add user management settings tab - ([27e2112](https://github.com/szinn/BookBoss/commit/27e2112c52184456454b8432504a4cd94919f352))
- _(core,frontend)_ Add clear rating button on book detail page - ([1d687a2](https://github.com/szinn/BookBoss/commit/1d687a277b8bdc44acd68a821337a75c344d4b85))
- _(frontend)_ User management, force password change, and polish - ([6782866](https://github.com/szinn/BookBoss/commit/67828667cf862cbedea502b0f15ba0171467dd58))

### Bug Fixes

- _(frontend)_ Restart shelf book resource when navigating between shelves - ([bb146e5](https://github.com/szinn/BookBoss/commit/bb146e57de80d12ba7d2f0bcc0cb753aac829cc3))

## [0.2.2](https://github.com/szinn/BookBoss/compare/v0.2.1..v0.2.2) - 2026-03-09

### Features

- _(core,frontend)_ Replace Dnf with Paused/Rereading/Abandoned reading statuses - ([8a2adb5](https://github.com/szinn/BookBoss/commit/8a2adb5d30e821f097d684f82b609dc5bdbb382b))

### Refactor

- _(frontend)_ Trim book detail reading panel and reorder sections - ([626e779](https://github.com/szinn/BookBoss/commit/626e77973d49eec17a8aaf70adaee17a6d8adb9c))

## [0.2.1](https://github.com/szinn/BookBoss/compare/v0.2.0..v0.2.1) - 2026-03-09

### Features

- _(core)_ Add ReadingService with state machine - ([7c4ac3d](https://github.com/szinn/BookBoss/commit/7c4ac3ddf30bf4f2128567952daca8d66cab68ee))
- _(database)_ Add UserBookMetadataRepository adapter - ([4fa283f](https://github.com/szinn/BookBoss/commit/4fa283f682b6a7d7fcced9dc768a5270a1035f25))
- _(frontend)_ Add Currently Reading section to books page - ([565a677](https://github.com/szinn/BookBoss/commit/565a677492ba989eb2d7262de256256f97f8f643))
- _(frontend)_ Add reading status badges to book cards - ([03270a8](https://github.com/szinn/BookBoss/commit/03270a831aa581be0c858f83ce2929f04e62ac85))
- _(frontend)_ Add reading controls to book detail page - ([41edaf6](https://github.com/szinn/BookBoss/commit/41edaf6523e9febe4927e02b1a5c487da852daee))
- _(frontend)_ Add auto-read threshold setting - ([aa3d1c0](https://github.com/szinn/BookBoss/commit/aa3d1c0ceee41447e22ba3e804ed857b0c086563))

### Refactor

- _(core)_ Sort capabilities - ([e31ae14](https://github.com/szinn/BookBoss/commit/e31ae149ff2cb4c18b94b64cb8794c67ef0bcabb))
- _(frontend)_ Match shelf bar and Currently Reading backgrounds to book grid - ([c570a5b](https://github.com/szinn/BookBoss/commit/c570a5b575707ff7813bdeb87329e9140454839d))

### Testing

- _(integration)_ Added MariaDB integration tests - ([a2c87eb](https://github.com/szinn/BookBoss/commit/a2c87ebf14f4af388444e0238150abb3ab2691f6))

## [0.2.0](https://github.com/szinn/BookBoss/compare/v0.1.16..v0.2.0) - 2026-03-08

### Features

- _(core)_ Add ShelfService trait and ShelfServiceImpl - ([762c6ec](https://github.com/szinn/BookBoss/commit/762c6ec7f9f38808448e54bcbfdca651da6dca6d))
- _(core,database,frontend)_ Add genre/tag fields and picklist data structures - ([e450235](https://github.com/szinn/BookBoss/commit/e4502359fb309ac7d249a7e5a0d102d003cc074c))
- _(database)_ Implement ShelfRepository adapter (M4.1) - ([d5a3e8c](https://github.com/szinn/BookBoss/commit/d5a3e8c2c22cb764490930e88b339de257746261))
- _(frontend)_ Add book file download endpoint and format download buttons - ([3df59c5](https://github.com/szinn/BookBoss/commit/3df59c5779ee3a94c0f7594c6de6cbad98abbeb0))
- _(frontend)_ Remove grid/table toggle button from NavBar - ([db0f1b9](https://github.com/szinn/BookBoss/commit/db0f1b98e3085756c358e10527a931858d77251e))
- _(frontend)_ Add DnD success flash, rename Edit buttons (M4.9) - ([3887d06](https://github.com/szinn/BookBoss/commit/3887d06432c1ef30d3c397c5b23f331d8389ebca))
- _(frontend)_ Drag-and-drop books onto shelf pills (D.1–D.3) - ([b635ecd](https://github.com/szinn/BookBoss/commit/b635ecde691341c147e5259bcac75df599101e49))
- _(frontend)_ Unified ShelfBar+BookGrid UX redesign (C.1–C.6) - ([36c179a](https://github.com/szinn/BookBoss/commit/36c179a15baed59c19a95f738762e13dd43a4e63))
- _(frontend)_ Add new shelf modal with name validation - ([a9cfbfa](https://github.com/szinn/BookBoss/commit/a9cfbfa3df0045d2231cd36516b3da9ea8d20334))
- _(frontend)_ Add shelves sidebar bar and ShelfPage route (M4.6) - ([d42c25a](https://github.com/szinn/BookBoss/commit/d42c25ad3e47a5402de99f7e11ef4294aa3676fa))
- _(frontend)_ Add books_for_shelf server function (M4.5) - ([095104d](https://github.com/szinn/BookBoss/commit/095104d71276f9a7aa6438275ba9792377d0c635))
- _(frontend)_ Add shelf CRUD server functions (M4.4) - ([0d8bb8c](https://github.com/szinn/BookBoss/commit/0d8bb8caea45e2345526ff7a7272a283b8dac5b0))
- _(frontend)_ Add ChipInput/AutocompleteInput components for picklist support - ([4f175c0](https://github.com/szinn/BookBoss/commit/4f175c063cf78f7f9fc8d0fcfbb7777c3dfb5364))
- _(frontend,core)_ UX redesign prep — update_shelf, BookSummary unification (A.1, A.2, B.1, B.3) - ([86adfd8](https://github.com/szinn/BookBoss/commit/86adfd8ec23b2b7776e24d9d0fef618d0f5bc7e6))
- _(metadata)_ Add Google Books metadata provider - ([ba57ce3](https://github.com/szinn/BookBoss/commit/ba57ce3493503d6bcb52918d2b40bed86afe300d))

### Bug Fixes

- _(database)_ Create jobs_claim index via SeaORM DSL instead of raw SQL - ([61d17f6](https://github.com/szinn/BookBoss/commit/61d17f64bbd4fbb67860f58943c3e6ea156663c3))
- _(database)_ Use regular index for jobs_claim on MySQL - ([c32c246](https://github.com/szinn/BookBoss/commit/c32c2468cf01779cbb5ce7ad22198e94d9cfaff0))
- _(frontend)_ Add 'All Books' entry to Shelves section in table view explorer - ([9cbae86](https://github.com/szinn/BookBoss/commit/9cbae868183964a5148b01c667dbe93368c28fe6))
- _(frontend)_ Change books_for_shelf server fn from GET to POST - ([d964ef0](https://github.com/szinn/BookBoss/commit/d964ef09977c47a901de87e259b14d40bd8d729c))
- _(frontend)_ Fix auth gaps in server functions - ([2e17165](https://github.com/szinn/BookBoss/commit/2e17165235a4e4d2ef923855b6c38c92eae23631))

### Refactor

- _(frontend)_ Split review_page into module with types/server/editor submodules - ([a47feee](https://github.com/szinn/BookBoss/commit/a47feee5b2d166e20037d563cb6a53c87c35f78c))

### Documentation

- Update README and docs to reflect current feature set - ([2c1879c](https://github.com/szinn/BookBoss/commit/2c1879cdffdee6cc76b2e2bff094b91ac977baff))

### Testing

- _(core)_ Add component tests for ShelfService (M4.3) - ([0a41188](https://github.com/szinn/BookBoss/commit/0a41188e44a78753cf9ab273fcc2ff7d856b07ab))
- _(integration)_ Add integration tests for book, import_job, and library services - ([a5e4584](https://github.com/szinn/BookBoss/commit/a5e4584e1ae28f068630eeee243ce0961a7405db))

### Miscellaneous Tasks

- _(database)_ Add missing indicies - ([fde9d54](https://github.com/szinn/BookBoss/commit/fde9d544792c1e29e47a435541c6339c866650ce))

## [0.1.15](https://github.com/szinn/BookBoss/compare/v0.1.14..v0.1.15) - 2026-03-07

### Features

- _(core,frontend)_ Add LibraryService with stats and delete book - ([0a2152e](https://github.com/szinn/BookBoss/commit/0a2152e8a8e1289eb67249d29890869967d80fef))
- _(core,frontend)_ Edit metadata page for library books - ([ae1425a](https://github.com/szinn/BookBoss/commit/ae1425a41f91999a6246a8e021f34799558ba949))
- _(import)_ Separate scan and worker poll intervals - ([6261611](https://github.com/szinn/BookBoss/commit/62616111a1b8b16eb5012c2eee2ff538b5413a75))

### Bug Fixes

- _(frontend)_ Refresh NavBar pending count after approve/reject - ([5f086fe](https://github.com/szinn/BookBoss/commit/5f086fefb7aa5f8c4935d3a5e445619f42dcbc3c))

### Refactor

- _(core)_ Decouple fetch_from_provider from ImportJobToken - ([acdd94d](https://github.com/szinn/BookBoss/commit/acdd94dc9b810be00aa55d31c8b9cb8bc64e2855))

### Documentation

- _(claude)_ Restructure CLAUDE.md files for clarity and reuse - ([2869eb3](https://github.com/szinn/BookBoss/commit/2869eb3bc99ca4a0c2fc65eac5cc8e72c2f303a7))
- _(user)_ Update import configuration reference - ([2746432](https://github.com/szinn/BookBoss/commit/27464325887dd9aeab0e03aac0692455444650fe))

## [0.1.14](https://github.com/szinn/BookBoss/compare/v0.1.12..v0.1.14) - 2026-03-07

### Features

- _(bookboss)_ Wire scanner and worker subsystems into server startup - ([c919c74](https://github.com/szinn/BookBoss/commit/c919c741338360fab67117d8aa8cd0d369cb3516))
- _(core)_ Add ApproveImports capability - ([6b4c2a2](https://github.com/szinn/BookBoss/commit/6b4c2a2a3203877cdeeda22573d7e37a692c8717))
- _(core)_ Independent metadata and cover selection across providers - ([8c11159](https://github.com/szinn/BookBoss/commit/8c111596552513dd7a086ebde84681558e6f6a25))
- _(core)_ Include first author in stored book file slug - ([9ef8b0b](https://github.com/szinn/BookBoss/commit/9ef8b0b39b21280a6ad08e6d83e3d2562ea08f2c))
- _(core,database,frontend)_ Book review and approval page (M3.22) - ([e0a4065](https://github.com/szinn/BookBoss/commit/e0a406535550e772af0c979e1a19ad7d348965a7))
- _(core,database,frontend)_ Incoming books list with reject cleanup - ([e0416a8](https://github.com/szinn/BookBoss/commit/e0416a838e295a7582d3f27a863b8d66f88de288))
- _(core,frontend)_ Best-resolution cover selection; review page dimensions - ([20b99d4](https://github.com/szinn/BookBoss/commit/20b99d4afe4753d226969c267f2e767cfb6bf2eb))
- _(core,frontend)_ Provider search uses current form identifiers - ([db6b936](https://github.com/szinn/BookBoss/commit/db6b936515e82c3003a43df477bb11a0ee34e3a8))
- _(database)_ Case-insensitive find_by_name across entity adapters - ([b7d22f8](https://github.com/szinn/BookBoss/commit/b7d22f8ae60fae0e73e2dfc7905eadbd203147c1))
- _(database)_ Add JobRepositoryAdapter with optimistic-locking claim loop - ([d6ec64a](https://github.com/szinn/BookBoss/commit/d6ec64a419f73cee44cd1f47a2b6dccc9cb0d92e))
- _(formats,core)_ Extract cover image from EPUB file - ([3cbf48c](https://github.com/szinn/BookBoss/commit/3cbf48cdd9141eda19dd503f678d9f684d278993))
- _(frontend)_ Simplify cover serving — token-only endpoint with blank fallback - ([996e565](https://github.com/szinn/BookBoss/commit/996e5658201ddf1c90631933a92aba1dc35d0b79))
- _(frontend,import)_ Pending count badge on Incoming nav link; fix tracing field - ([4b3e40e](https://github.com/szinn/BookBoss/commit/4b3e40ede88006fbe7aedfacea9add489171e72b))
- _(import)_ Add bb-import crate with scanner subsystem and process_import handler - ([8b0f9b4](https://github.com/szinn/BookBoss/commit/8b0f9b454bc2fa98dd8683696ec3adb1e5aa8e08))

### Bug Fixes

- _(api)_ Enable TLS for gRPC client connections - ([75a660c](https://github.com/szinn/BookBoss/commit/75a660c85189a0b84960243475b8617e72b4f440))
- _(import,core)_ Pipeline robustness and startup recovery - ([30d4825](https://github.com/szinn/BookBoss/commit/30d48251d2c1080b36346d64f645e2db0cb5e8c5))

### Documentation

- Add storage crate and M3 config vars to README and docs - ([84e56ea](https://github.com/szinn/BookBoss/commit/84e56ea158d1a29a78061df9c6a64d955a82fe19))

### Miscellaneous Tasks

- _(core,import)_ Improve subsystem and pipeline tracing - ([6396e4d](https://github.com/szinn/BookBoss/commit/6396e4d18d5649a886bf72289ebf642478e2bab8))

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
