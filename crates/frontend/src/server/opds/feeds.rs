//! OPDS feed handlers for root catalog, all books, shelves, and file serving.

use std::sync::Arc;

use axum::{
    Extension,
    body::Body,
    extract::{Path, Query},
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use bb_core::{
    CoreServices,
    book::{AuthorToken, Book, BookQuery, BookToken, FileFormat, FileRole, SeriesToken},
    filter::{BookFilter, FilterRule, TextOp},
    library::LibraryToken,
    shelf::ShelfType,
};
use chrono::Utc;
use serde::Deserialize;

use super::{
    extractor::OpdsUser,
    xml::{AtomEntry, AtomFeed, AtomLink, mime, rel},
};

const PAGE_SIZE: u64 = 50;

#[derive(Deserialize)]
pub struct PaginationParams {
    pub start: Option<u64>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub start: Option<u64>,
}

fn xml_response(xml: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(mime::ATOM_XML))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"))
        .body(axum::body::Body::from(xml))
        .unwrap()
}

/// `GET /opds/` — Root catalog (navigation feed).
pub async fn root(opds_user: OpdsUser, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let now = Utc::now();
    let user_id = opds_user.user.id;

    let libs = core_services.library_service.libraries_for_user(user_id).await.unwrap_or_default();

    let mut feed = AtomFeed::new("urn:bookboss:opds:root", "BookBoss Catalog", now)
        .with_link(AtomLink::new(rel::SELF, "/opds/").with_type(mime::NAVIGATION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION))
        .with_link(
            AtomLink::new(rel::SEARCH, "/opds/search/description.xml")
                .with_type(mime::OPENSEARCH)
                .with_title("Search BookBoss"),
        )
        .with_entry(
            AtomEntry::new("urn:bookboss:opds:all", "All Books", now)
                .with_content("Browse books in your default library")
                .with_link(AtomLink::new(rel::SUBSECTION, "/opds/all").with_type(mime::ACQUISITION)),
        );

    if libs.len() >= 2 {
        feed = feed.with_entry(
            AtomEntry::new("urn:bookboss:opds:libraries", "Libraries", now)
                .with_content("Browse books by library")
                .with_link(AtomLink::new(rel::SUBSECTION, "/opds/libraries").with_type(mime::NAVIGATION)),
        );
    }

    feed = feed
        .with_entry(
            AtomEntry::new("urn:bookboss:opds:shelves", "Shelves", now)
                .with_content("Browse books by shelf")
                .with_link(AtomLink::new(rel::SUBSECTION, "/opds/shelves").with_type(mime::NAVIGATION)),
        )
        .with_entry(
            AtomEntry::new("urn:bookboss:opds:authors", "Authors", now)
                .with_content("Browse books by author")
                .with_link(AtomLink::new(rel::SUBSECTION, "/opds/authors").with_type(mime::NAVIGATION)),
        )
        .with_entry(
            AtomEntry::new("urn:bookboss:opds:series", "Series", now)
                .with_content("Browse books by series")
                .with_link(AtomLink::new(rel::SUBSECTION, "/opds/series").with_type(mime::NAVIGATION)),
        );

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::empty())
            .unwrap(),
    }
}

/// `GET /opds/all` — Books in the user's default library (acquisition feed,
/// paginated).
pub async fn all_books(opds_user: OpdsUser, Query(params): Query<PaginationParams>, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let now = Utc::now();
    let user_id = opds_user.user.id;

    // Resolve default library — return 500 rather than silently expanding to all
    // books
    let library_id = match core_services.library_service.get_default_library_token(user_id).await {
        Ok(token_str) => match LibraryToken::parse(&token_str) {
            Ok(token) => Some(token.id()),
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let filter = BookQuery::default();

    let offset = params.start;
    let Ok(books) = core_services.book_service.list_books(&filter, library_id, offset, Some(PAGE_SIZE + 1)).await else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::empty())
            .unwrap();
    };

    let has_next = books.len() as u64 > PAGE_SIZE;
    let page_books = if has_next { &books[..PAGE_SIZE as usize] } else { &books };

    let mut feed = AtomFeed::new("urn:bookboss:opds:all", "All Books", now)
        .with_link(AtomLink::new(rel::SELF, format_all_url(offset)).with_type(mime::ACQUISITION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next {
        let next_offset = offset.unwrap_or(0) + PAGE_SIZE;
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_all_url(Some(next_offset))).with_type(mime::ACQUISITION));
    }

    for book in page_books {
        let entry = book_to_entry(book, &core_services).await;
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::empty())
            .unwrap(),
    }
}

/// `GET /opds/libraries` — User's assigned libraries (navigation feed).
/// Only includes a Libraries entry in the root when user has 2+ libraries.
pub async fn libraries(opds_user: OpdsUser, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let now = Utc::now();
    let user_id = opds_user.user.id;

    let Ok(libs) = core_services.library_service.libraries_for_user(user_id).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let mut feed = AtomFeed::new("urn:bookboss:opds:libraries", "Libraries", now)
        .with_link(AtomLink::new(rel::SELF, "/opds/libraries").with_type(mime::NAVIGATION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    for lib in &libs {
        let token_str = lib.token.to_string();
        let entry = AtomEntry::new(format!("urn:bookboss:library:{token_str}"), &lib.name, now)
            .with_link(AtomLink::new(rel::SUBSECTION, format!("/opds/libraries/{token_str}")).with_type(mime::ACQUISITION));
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/libraries/{token}` — Books in a specific library (acquisition
/// feed, paginated).
pub async fn library_books(
    opds_user: OpdsUser,
    Path(library_token_str): Path<String>,
    Query(params): Query<PaginationParams>,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let now = Utc::now();
    let user_id = opds_user.user.id;

    let library_token: LibraryToken = match LibraryToken::parse(&library_token_str) {
        Ok(t) => t,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };
    let library_id = library_token.id();

    // Access check
    if core_services.library_service.validate_user_library_access(user_id, library_id).await.is_err() {
        return error_response(StatusCode::FORBIDDEN);
    }

    // Get library name for feed title
    let library_name = match core_services.library_service.libraries_for_user(user_id).await {
        Ok(libs) => libs
            .into_iter()
            .find(|l| l.id == library_id)
            .map_or_else(|| library_token_str.clone(), |l| l.name),
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let filter = BookQuery::default();
    let offset = params.start;
    let Ok(books) = core_services
        .book_service
        .list_books(&filter, Some(library_id), offset, Some(PAGE_SIZE + 1))
        .await
    else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let has_next = books.len() as u64 > PAGE_SIZE;
    let page_books = if has_next { &books[..PAGE_SIZE as usize] } else { &books };
    let self_url = format!("/opds/libraries/{library_token_str}");

    let mut feed = AtomFeed::new(format!("urn:bookboss:library:{library_token_str}"), &library_name, now)
        .with_link(AtomLink::new(rel::SELF, format_paginated_url(&self_url, offset)).with_type(mime::ACQUISITION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next {
        let next_offset = offset.unwrap_or(0) + PAGE_SIZE;
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_paginated_url(&self_url, Some(next_offset))).with_type(mime::ACQUISITION));
    }

    for book in page_books {
        let entry = book_to_entry(book, &core_services).await;
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/shelves` — User's shelves (navigation feed).
pub async fn shelves(opds_user: OpdsUser, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let now = Utc::now();

    let Ok(shelf_list) = core_services.shelf_service.list_shelves_for_user(opds_user.user.id).await else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::empty())
            .unwrap();
    };

    let mut feed = AtomFeed::new("urn:bookboss:opds:shelves", "Shelves", now)
        .with_link(AtomLink::new(rel::SELF, "/opds/shelves").with_type(mime::NAVIGATION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    for shelf in &shelf_list {
        let entry = AtomEntry::new(format!("urn:bookboss:shelf:{}", shelf.token), &shelf.name, shelf.updated_at)
            .with_link(AtomLink::new(rel::SUBSECTION, format!("/opds/shelves/{}", shelf.token)).with_type(mime::ACQUISITION));
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::empty())
            .unwrap(),
    }
}

fn format_all_url(start: Option<u64>) -> String {
    format_paginated_url("/opds/all", start)
}

/// Adds acquisition links for available book files.
///
/// Only EPUB files are included — KEPUB is a Kobo-specific container and
/// not suitable for generic OPDS clients. Enriched files take priority
/// over originals.
pub(crate) fn add_file_links(mut entry: AtomEntry, book_token: &str, files: &[bb_core::book::BookFile]) -> AtomEntry {
    let epub_ext = FileFormat::Epub.extension();
    let epub_type = FileFormat::Epub.content_type();

    let enriched = files.iter().find(|f| f.format == FileFormat::Epub && f.file_role == FileRole::Enriched);
    let original = files.iter().find(|f| f.format == FileFormat::Epub && f.file_role == FileRole::Original);

    if enriched.or(original).is_some() {
        entry = entry.with_link(AtomLink::new(rel::ACQUISITION, format!("/opds/download/{book_token}/{epub_ext}")).with_type(epub_type));
    }
    entry
}

/// Adds cover image link if the book has a cover.
pub(crate) fn add_cover_link(mut entry: AtomEntry, book_token: &str, has_cover: bool) -> AtomEntry {
    if has_cover {
        let full_cover_url = format!("/opds/covers/{book_token}?full=true");
        let thumbnail_url = format!("/opds/covers/{book_token}");
        entry = entry
            .with_link(AtomLink::new(rel::IMAGE, &full_cover_url).with_type("image/jpeg"))
            .with_link(AtomLink::new(rel::THUMBNAIL, &thumbnail_url).with_type("image/jpeg"));
    }
    entry
}

/// `GET /opds/shelves/{shelf_token}` — Books on a shelf (acquisition feed).
pub async fn shelf_books(
    opds_user: OpdsUser,
    axum::extract::Path(shelf_token_str): axum::extract::Path<String>,
    Query(params): Query<PaginationParams>,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let now = Utc::now();
    let user_id = opds_user.user.id;

    let shelf_token: bb_core::shelf::ShelfToken = match shelf_token_str.parse() {
        Ok(t) => t,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };

    let Ok(shelf) = core_services.shelf_service.get_shelf(shelf_token, user_id).await else {
        return error_response(StatusCode::NOT_FOUND);
    };

    let offset = params.start;
    let books: Vec<Book> = if shelf.shelf_type == ShelfType::Smart {
        match core_services
            .shelf_service
            .books_for_filter(shelf_token, user_id, offset, Some(PAGE_SIZE + 1), None)
            .await
        {
            Ok(b) => b,
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        let Ok(entries) = core_services
            .shelf_service
            .books_for_shelf(shelf_token, user_id, offset, Some(PAGE_SIZE + 1))
            .await
        else {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR);
        };
        let mut result = Vec::with_capacity(entries.len());
        for entry in &entries {
            if let Ok(Some(book)) = core_services.book_service.find_book_by_token(BookToken::new(entry.book_id)).await {
                result.push(book);
            }
        }
        result
    };

    let has_next = books.len() as u64 > PAGE_SIZE;
    let page_books = if has_next { &books[..PAGE_SIZE as usize] } else { &books };

    let self_url = format_shelf_url(&shelf_token_str, offset);
    let mut feed = AtomFeed::new(format!("urn:bookboss:shelf:{}", shelf.token), &shelf.name, now)
        .with_link(AtomLink::new(rel::SELF, &self_url).with_type(mime::ACQUISITION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next {
        let next_offset = offset.unwrap_or(0) + PAGE_SIZE;
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_shelf_url(&shelf_token_str, Some(next_offset))).with_type(mime::ACQUISITION));
    }

    for book in page_books {
        let entry = book_to_entry(book, &core_services).await;
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/authors` — Authors (navigation feed, paginated).
pub async fn authors(opds_user: OpdsUser, Query(params): Query<PaginationParams>, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let _ = &opds_user;
    let now = Utc::now();

    let Ok(author_list) = core_services.book_service.list_authors(params.start, Some(PAGE_SIZE + 1)).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let has_next = author_list.len() as u64 > PAGE_SIZE;
    let page_authors = if has_next { &author_list[..PAGE_SIZE as usize] } else { &author_list };

    let mut feed = AtomFeed::new("urn:bookboss:opds:authors", "Authors", now)
        .with_link(AtomLink::new(rel::SELF, format_paginated_url("/opds/authors", params.start)).with_type(mime::NAVIGATION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next && let Some(last) = page_authors.last() {
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_paginated_url("/opds/authors", Some(last.id + 1))).with_type(mime::NAVIGATION));
    }

    for author in page_authors {
        let entry = AtomEntry::new(format!("urn:bookboss:author:{}", author.token), &author.name, author.updated_at)
            .with_link(AtomLink::new(rel::SUBSECTION, format!("/opds/authors/{}", author.id)).with_type(mime::ACQUISITION));
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/authors/{id}` — Books by author (acquisition feed, paginated).
pub async fn author_books(
    opds_user: OpdsUser,
    Path(author_id): Path<u64>,
    Query(params): Query<PaginationParams>,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let _ = &opds_user;
    let now = Utc::now();

    let author = match core_services.book_service.find_author_by_token(AuthorToken::new(author_id)).await {
        Ok(Some(a)) => a,
        Ok(None) => return error_response(StatusCode::NOT_FOUND),
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let filter = BookQuery {
        author_id: Some(author_id),
        ..Default::default()
    };

    let offset = params.start;
    let Ok(books) = core_services.book_service.list_books(&filter, None, offset, Some(PAGE_SIZE + 1)).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let has_next = books.len() as u64 > PAGE_SIZE;
    let page_books = if has_next { &books[..PAGE_SIZE as usize] } else { &books };
    let base_url = format!("/opds/authors/{author_id}");

    let mut feed = AtomFeed::new(format!("urn:bookboss:author:{}", author.token), &author.name, now)
        .with_link(AtomLink::new(rel::SELF, format_paginated_url(&base_url, offset)).with_type(mime::ACQUISITION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next {
        let next_offset = offset.unwrap_or(0) + PAGE_SIZE;
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_paginated_url(&base_url, Some(next_offset))).with_type(mime::ACQUISITION));
    }

    for book in page_books {
        let entry = book_to_entry(book, &core_services).await;
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/series` — Series (navigation feed, paginated).
pub async fn series_list(opds_user: OpdsUser, Query(params): Query<PaginationParams>, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let _ = &opds_user;
    let now = Utc::now();

    let Ok(all_series) = core_services.book_service.list_series(params.start, Some(PAGE_SIZE + 1)).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let has_next = all_series.len() as u64 > PAGE_SIZE;
    let page_series = if has_next { &all_series[..PAGE_SIZE as usize] } else { &all_series };

    let mut feed = AtomFeed::new("urn:bookboss:opds:series", "Series", now)
        .with_link(AtomLink::new(rel::SELF, format_paginated_url("/opds/series", params.start)).with_type(mime::NAVIGATION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next && let Some(last) = page_series.last() {
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_paginated_url("/opds/series", Some(last.id + 1))).with_type(mime::NAVIGATION));
    }

    for series in page_series {
        let entry = AtomEntry::new(format!("urn:bookboss:series:{}", series.token), &series.name, series.updated_at)
            .with_link(AtomLink::new(rel::SUBSECTION, format!("/opds/series/{}", series.id)).with_type(mime::ACQUISITION));
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/series/{id}` — Books in a series (acquisition feed, paginated).
pub async fn series_books(
    opds_user: OpdsUser,
    Path(series_id): Path<u64>,
    Query(params): Query<PaginationParams>,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let _ = &opds_user;
    let now = Utc::now();

    let series = match core_services.book_service.find_series_by_token(SeriesToken::new(series_id)).await {
        Ok(Some(s)) => s,
        Ok(None) => return error_response(StatusCode::NOT_FOUND),
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let filter = BookQuery {
        series_id: Some(series_id),
        ..Default::default()
    };

    let offset = params.start;
    let Ok(books) = core_services.book_service.list_books(&filter, None, offset, Some(PAGE_SIZE + 1)).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let has_next = books.len() as u64 > PAGE_SIZE;
    let page_books = if has_next { &books[..PAGE_SIZE as usize] } else { &books };
    let base_url = format!("/opds/series/{series_id}");

    let mut feed = AtomFeed::new(format!("urn:bookboss:series:{}", series.token), &series.name, now)
        .with_link(AtomLink::new(rel::SELF, format_paginated_url(&base_url, offset)).with_type(mime::ACQUISITION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next {
        let next_offset = offset.unwrap_or(0) + PAGE_SIZE;
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_paginated_url(&base_url, Some(next_offset))).with_type(mime::ACQUISITION));
    }

    for book in page_books {
        let entry = book_to_entry(book, &core_services).await;
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/search/description.xml` — OpenSearch description document.
pub async fn search_description(_opds_user: OpdsUser) -> Response {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
  <ShortName>BookBoss</ShortName>
  <Description>Search the BookBoss library by title or author</Description>
  <Url type="application/atom+xml;profile=opds-catalog;kind=acquisition"
       template="/opds/search?q={searchTerms}&amp;start={startIndex?}"/>
</OpenSearchDescription>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(mime::OPENSEARCH))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"))
        .body(Body::from(xml))
        .unwrap()
}

/// `GET /opds/search?q=...` — Search acquisition feed.
pub async fn search(opds_user: OpdsUser, Query(params): Query<SearchParams>, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let _ = &opds_user;
    let now = Utc::now();

    let q = match params.q.as_deref().filter(|s| !s.is_empty()) {
        Some(q) => q.to_string(),
        None => {
            return xml_response(
                AtomFeed::new("urn:bookboss:opds:search", "Search Results", now)
                    .with_link(AtomLink::new(rel::SELF, "/opds/search").with_type(mime::ACQUISITION))
                    .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION))
                    .to_xml()
                    .unwrap_or_default(),
            );
        }
    };

    let filter = BookFilter::Rule(FilterRule::TitleText {
        op: TextOp::Contains,
        value: q.clone(),
    })
    .or(BookFilter::Rule(FilterRule::AuthorText {
        op: TextOp::Contains,
        value: q.clone(),
    }));

    let offset = params.start;
    let Ok(books) = core_services.collection_service.search_books(&filter, None, offset, Some(PAGE_SIZE + 1)).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let has_next = books.len() as u64 > PAGE_SIZE;
    let page_books = if has_next { &books[..PAGE_SIZE as usize] } else { &books };

    let self_url = format_search_url(&q, offset);
    let mut feed = AtomFeed::new("urn:bookboss:opds:search", format!("Search: {q}"), now)
        .with_link(AtomLink::new(rel::SELF, &self_url).with_type(mime::ACQUISITION))
        .with_link(AtomLink::new(rel::START, "/opds/").with_type(mime::NAVIGATION));

    if has_next {
        let next_offset = offset.unwrap_or(0) + PAGE_SIZE;
        feed = feed.with_link(AtomLink::new(rel::NEXT, format_search_url(&q, Some(next_offset))).with_type(mime::ACQUISITION));
    }

    for book in page_books {
        let entry = book_to_entry(book, &core_services).await;
        feed = feed.with_entry(entry);
    }

    match feed.to_xml() {
        Ok(xml) => xml_response(xml),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn format_search_url(q: &str, start: Option<u64>) -> String {
    let encoded_q = q.replace('&', "%26").replace(' ', "+");
    match start {
        Some(s) => format!("/opds/search?q={encoded_q}&start={s}"),
        None => format!("/opds/search?q={encoded_q}"),
    }
}

fn format_paginated_url(base: &str, start: Option<u64>) -> String {
    match start {
        Some(s) => format!("{base}?start={s}"),
        None => base.to_string(),
    }
}

fn format_shelf_url(token: &str, start: Option<u64>) -> String {
    format_paginated_url(&format!("/opds/shelves/{token}"), start)
}

fn error_response(status: StatusCode) -> Response {
    Response::builder().status(status).body(axum::body::Body::empty()).unwrap()
}

/// Builds an OPDS acquisition entry from a Book, resolving authors, files, and
/// cover.
async fn book_to_entry(book: &Book, core_services: &Arc<CoreServices>) -> AtomEntry {
    let mut book_authors = core_services.book_service.authors_for_book(book.id).await.unwrap_or_default();
    book_authors.sort_by_key(|a| a.sort_order);
    let files = core_services.book_service.files_for_book(book.id).await.unwrap_or_default();

    let mut entry = AtomEntry::new(format!("urn:bookboss:book:{}", book.token), &book.title, book.updated_at);

    if let Some(ref desc) = book.description {
        entry = entry.with_content(desc);
    }

    for ba in &book_authors {
        if let Ok(Some(author)) = core_services.book_service.find_author_by_token(AuthorToken::new(ba.author_id)).await {
            entry = entry.with_author(&author.name);
        }
    }

    let token_str = book.token.to_string();
    entry = add_file_links(entry, &token_str, &files);
    entry = add_cover_link(entry, &token_str, book.has_cover);

    entry
}

static BLANK_COVER: &[u8] = include_bytes!("../../../assets/BlankCover.png");

#[derive(Deserialize)]
pub struct CoverParams {
    pub full: Option<bool>,
}

/// `GET /opds/covers/{book_token}` — Serve a book's cover image.
///
/// Without query params (or `?full=false`) returns the thumbnail. Pass
/// `?full=true` to get the full-resolution cover.
pub async fn serve_cover(
    Path(book_token_str): Path<String>,
    Query(params): Query<CoverParams>,
    _opds_user: OpdsUser,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let token: BookToken = match book_token_str.parse() {
        Ok(t) => t,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };

    let book = match core_services.book_service.find_book_by_token(token).await {
        Ok(Some(b)) => b,
        Ok(None) => return error_response(StatusCode::NOT_FOUND),
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if !book.has_cover {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
            .header(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))
            .body(Body::from(BLANK_COVER))
            .unwrap();
    }

    let path = if params.full.unwrap_or(false) {
        core_services.file_store.cover_path(token)
    } else {
        core_services.file_store.thumbnail_path(token)
    };

    match tokio::fs::read(&path).await {
        Ok(data) => {
            let content_type = "image/jpeg";
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
                .header(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))
                .body(Body::from(data))
                .unwrap()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
            .header(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))
            .body(Body::from(BLANK_COVER))
            .unwrap(),
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// `GET /opds/download/{book_token}/{format}` — Download a book file.
pub async fn serve_download(
    Path((book_token_str, format_str)): Path<(String, String)>,
    _opds_user: OpdsUser,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let token: BookToken = match book_token_str.parse() {
        Ok(t) => t,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };

    let format: FileFormat = match format_str.to_lowercase().parse() {
        Ok(f) => f,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };

    let book = match core_services.book_service.find_book_by_token(token).await {
        Ok(Some(b)) => b,
        Ok(None) => return error_response(StatusCode::NOT_FOUND),
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let Ok(files) = core_services.book_service.files_for_book(book.id).await else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let enriched_file = files.iter().find(|f| f.format == format && f.file_role == FileRole::Enriched);
    let original_file = files.iter().find(|f| f.format == format && f.file_role == FileRole::Original);

    if enriched_file.is_none() && original_file.is_none() {
        return error_response(StatusCode::NOT_FOUND);
    }

    let ext = format.extension();

    // Try the enriched file first; fall back to the original if not yet on disk.
    let data = if let Some(enriched) = enriched_file {
        let enriched_path = core_services.file_store.resolve(&enriched.path);
        match tokio::fs::read(&enriched_path).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let Some(original) = original_file else {
                    return error_response(StatusCode::NOT_FOUND);
                };
                let orig_path = core_services.file_store.resolve(&original.path);
                match tokio::fs::read(&orig_path).await {
                    Ok(d) => d,
                    Err(_) => return error_response(StatusCode::NOT_FOUND),
                }
            }
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        let Some(original) = original_file else {
            return error_response(StatusCode::NOT_FOUND);
        };
        let orig_path = core_services.file_store.resolve(&original.path);
        match tokio::fs::read(&orig_path).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return error_response(StatusCode::NOT_FOUND),
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR),
        }
    };

    let download_name = format!("{}.{ext}", sanitize_filename(&book.title));
    let content_disposition = format!("attachment; filename=\"{download_name}\"");

    // Fire-and-forget hash registration for KOReader sync.
    {
        let svc = core_services.koreader_service.clone();
        let book_id = book.id;
        let filename = download_name.clone();
        let bytes = data.clone();
        tokio::spawn(async move {
            if let Err(e) = svc.register_hashes(book_id, &filename, &bytes).await {
                tracing::warn!("KOReader hash registration failed for book {book_id}: {e}");
            }
        })
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(format.content_type()))
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&content_disposition).unwrap_or(HeaderValue::from_static("attachment")),
        )
        .header(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"))
        .body(Body::from(data))
        .unwrap()
}

fn sanitize_filename(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect()
}
