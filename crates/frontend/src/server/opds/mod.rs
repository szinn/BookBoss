//! OPDS 1.x catalog server.
//!
//! Exposes the library as an OPDS Atom catalog for e-reader apps. All
//! endpoints live under `/opds/` and authenticate via HTTP Basic Auth using the
//! user's BookBoss username and their OPDS-specific password.
//!
//! # Endpoint map
//!
//! | Method | Path                              | Description              |
//! |--------|-----------------------------------|--------------------------|
//! | GET    | `/opds/`                          | Root catalog (nav feed)  |
//! | GET    | `/opds/all`                       | All books (acquisition)  |
//! | GET    | `/opds/shelves`                   | User shelves (nav feed)  |
//! | GET    | `/opds/shelves/:token`            | Shelf books (acquisition)|
//! | GET    | `/opds/authors`                   | Authors (nav feed)       |
//! | GET    | `/opds/authors/:id`               | Author books (acq feed)  |
//! | GET    | `/opds/series`                    | Series (nav feed)        |
//! | GET    | `/opds/series/:id`                | Series books (acq feed)  |
//! | GET    | `/opds/covers/:book_token`        | Cover image              |
//! | GET    | `/opds/download/:book_token/:fmt` | Book file download       |

pub mod extractor;
pub mod feeds;
pub mod xml;

use axum::{Router, routing};

/// Builds the OPDS catalog router.
pub fn opds_router() -> Router {
    Router::new()
        .route("/opds", routing::get(feeds::root))
        .route("/opds/", routing::get(feeds::root))
        .route("/opds/all", routing::get(feeds::all_books))
        .route("/opds/shelves", routing::get(feeds::shelves))
        .route("/opds/shelves/{shelf_token}", routing::get(feeds::shelf_books))
        .route("/opds/authors", routing::get(feeds::authors))
        .route("/opds/authors/{id}", routing::get(feeds::author_books))
        .route("/opds/series", routing::get(feeds::series_list))
        .route("/opds/series/{id}", routing::get(feeds::series_books))
        .route("/opds/covers/{book_token}", routing::get(feeds::serve_cover))
        .route("/opds/download/{book_token}/{format}", routing::get(feeds::serve_download))
}
