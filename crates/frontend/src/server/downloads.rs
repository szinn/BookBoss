use std::sync::Arc;

use axum::{
    Extension,
    body::Body,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use bb_core::{
    CoreServices,
    book::{BookToken, FileFormat, FileRole},
};

use super::AuthSession;

/// Serves a book file for download.
///
/// Route: `GET /api/v1/books/:book_token/download/:format`
///
/// Requires authentication. Resolves the book file path from the stored
/// `BookFile.path` field via `FileStoreService::resolve`, then streams the file
/// with a `Content-Disposition: attachment` header.
pub(crate) async fn serve_book_file(
    Path((book_token_str, format_str)): Path<(String, String)>,
    auth_session: AuthSession,
    Extension(core_services): Extension<Arc<CoreServices>>,
) -> Response {
    let authenticated = auth_session.current_user.as_ref().is_some_and(|u| !u.username.is_empty());
    if !authenticated {
        return Response::builder().status(StatusCode::UNAUTHORIZED).body(Body::empty()).unwrap();
    }

    let token: BookToken = match book_token_str.parse() {
        Ok(t) => t,
        Err(_) => return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()).unwrap(),
    };

    let Ok(format) = format_str.to_lowercase().parse::<FileFormat>() else {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()).unwrap();
    };

    let book = match core_services.book_service.find_book_by_token(token).await {
        Ok(Some(b)) => b,
        Ok(None) => return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap(),
        Err(e) if e.is_transient() => {
            return Response::builder().status(StatusCode::SERVICE_UNAVAILABLE).body(Body::empty()).unwrap();
        }
        Err(_) => return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap(),
    };

    // Load file records and split by role for the requested format.
    let files = match core_services.book_service.files_for_book(book.id).await {
        Ok(f) => f,
        Err(e) if e.is_transient() => {
            return Response::builder().status(StatusCode::SERVICE_UNAVAILABLE).body(Body::empty()).unwrap();
        }
        Err(_) => return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap(),
    };
    let enriched_file = files.iter().find(|f| f.format == format && f.file_role == FileRole::Enriched);
    let original_file = files.iter().find(|f| f.format == format && f.file_role == FileRole::Original);

    if enriched_file.is_none() && original_file.is_none() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))
            .body(Body::from("This file is not available and may be queued for regeneration."))
            .unwrap();
    }

    let ext = format.extension();

    // Try the enriched file first; fall back to the original if not yet on
    // disk.
    let data = if let Some(enriched) = enriched_file {
        let enriched_path = core_services.file_store.resolve(&enriched.path);
        match tokio::fs::read(&enriched_path).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Enriched record exists but file not on disk yet — fall back
                // to original.
                let Some(original) = original_file else {
                    return Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .header(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                        .body(Body::from("This file is not available and may be queued for regeneration."))
                        .unwrap();
                };
                let orig_path = core_services.file_store.resolve(&original.path);
                match tokio::fs::read(&orig_path).await {
                    Ok(d) => d,
                    Err(_) => return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap(),
                }
            }
            Err(_) => return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap(),
        }
    } else {
        // No enriched record — serve the original directly.
        let Some(original) = original_file else {
            return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap();
        };
        let orig_path = core_services.file_store.resolve(&original.path);
        match tokio::fs::read(&orig_path).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap();
            }
            Err(_) => return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::empty()).unwrap(),
        }
    };

    // Use the book title as the suggested download filename.
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

/// Produces a safe filename fragment from a book title (keeps alphanumerics,
/// spaces, and hyphens; replaces everything else with underscores).
fn sanitize_filename(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect()
}
