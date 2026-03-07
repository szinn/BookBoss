use std::sync::Arc;

use axum::{
    Extension,
    body::Body,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use bb_core::{CoreServices, book::BookToken};

static BLANK_COVER: &[u8] = include_bytes!("../../assets/BlankCover.png");

/// Serves a cover image for a given book token.
///
/// Route: `GET /api/v1/covers/:book_token`
///
/// Looks up the book's cover filename from the database, reads the file from
/// the library store, and returns it with the appropriate `Content-Type`
/// header. If the book has no cover, serves the built-in blank cover PNG.
pub(crate) async fn serve_cover(Path(book_token_str): Path<String>, Extension(core_services): Extension<Arc<CoreServices>>) -> Response {
    let token: BookToken = match book_token_str.parse() {
        Ok(t) => t,
        Err(_) => return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()).unwrap(),
    };

    let book = match core_services.book_service.find_book_by_token(&token).await {
        Ok(Some(b)) => b,
        Ok(None) => return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap(),
        Err(_) => return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap(),
    };

    let Some(filename) = book.cover_path else {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
            .header(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"))
            .body(Body::from(BLANK_COVER))
            .unwrap();
    };

    let path = core_services.library_store.cover_path(&token, &filename);

    let (data, content_type) = match tokio::fs::read(&path).await {
        Ok(d) => (d, content_type_for_filename(&filename)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))
                .header(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"))
                .body(Body::from(BLANK_COVER))
                .unwrap();
        }
        Err(_) => return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"))
        .body(Body::from(data))
        .unwrap()
}

fn content_type_for_filename(filename: &str) -> &'static str {
    let ext = filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}
