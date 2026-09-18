use bb_core::{
    Error, RepositoryError,
    book::{AuthorRole, BookQuery, BookStatus},
    collection::BookEdit,
    import::ImportStatus,
    pipeline::{ExtractedAuthor, ExtractedMetadata},
};

use crate::{fixtures, setup};

// ── Helpers
// ──────────────────────────────────────────────────────────────────────

/// Returns the default `ExtractedMetadata` used for most pipeline tests.
fn stub_metadata() -> ExtractedMetadata {
    ExtractedMetadata {
        title: Some("The Test Book".to_string()),
        authors: Some(vec![ExtractedAuthor {
            name: "Test Author".to_string(),
            role: Some(AuthorRole::Author),
            sort_order: 0,
        }]),
        ..Default::default()
    }
}

/// Returns a minimal `BookEdit` suitable for approve_job tests.
fn minimal_edit(title: &str, author: &str) -> BookEdit {
    BookEdit {
        title: title.to_string(),
        description: None,
        published_date: None,
        language: None,
        series_name: None,
        series_number: None,
        publisher_name: None,
        page_count: None,
        authors: vec![author.to_string()],
        identifiers: vec![],
        use_fetched_cover: false,
        genres: vec![],
        tags: vec![],
    }
}

// ── process_job tests
// ───────────────────────────────────────────────────────────

#[tokio::test]
async fn process_job_transitions_to_needs_review() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_needs_review").await;

    let updated = svc.pipeline_service.process_job(job).await.unwrap();

    assert_eq!(updated.status, ImportStatus::NeedsReview);
    assert!(updated.candidate_book_id.is_some(), "candidate_book_id must be set after processing");
}

#[tokio::test]
async fn process_job_creates_book_with_incoming_status() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_incoming").await;

    let updated = svc.pipeline_service.process_job(job).await.unwrap();

    let book_id = updated.candidate_book_id.expect("candidate_book_id set");
    let book = fixtures::find_book_by_id(&ctx.repos, book_id).await.expect("book found");
    assert_eq!(book.status, BookStatus::Incoming);
    assert_eq!(book.title, "The Test Book");
}

#[tokio::test]
async fn process_job_creates_author_from_metadata() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_author_created").await;

    let updated = svc.pipeline_service.process_job(job).await.unwrap();

    let book_id = updated.candidate_book_id.expect("candidate_book_id set");
    let book = fixtures::find_book_by_id(&ctx.repos, book_id).await.expect("book found");
    let authors = svc.book_service.authors_for_book(book.id).await.unwrap();
    assert_eq!(authors.len(), 1);
    // Verify the linked author is "Test Author" by looking up by author_id
    let all_authors = svc.book_service.list_all_authors().await.unwrap();
    let linked = all_authors.iter().find(|a| a.id == authors[0].author_id).expect("author found");
    assert_eq!(linked.name, "Test Author");
}

#[tokio::test]
async fn process_job_dedupes_duplicate_authors() {
    // Source metadata that lists the same person twice — e.g. duplicate
    // <dc:creator> entries with and without a role attribute. Both normalize
    // to the same author row; without dedup the second insert would violate
    // the (book_id, author_id) PK on book_authors.
    let metadata = ExtractedMetadata {
        title: Some("The Test Book".to_string()),
        authors: Some(vec![
            ExtractedAuthor {
                name: "Test Author".to_string(),
                role: Some(AuthorRole::Author),
                sort_order: 0,
            },
            ExtractedAuthor {
                name: "Test Author".to_string(),
                role: None,
                sort_order: 1,
            },
        ]),
        ..Default::default()
    };
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, metadata);
    let job = fixtures::insert_import_job(&ctx.repos, "hash_duplicate_authors").await;

    let updated = svc
        .pipeline_service
        .process_job(job)
        .await
        .expect("process_job must not fail on duplicate authors");

    let book_id = updated.candidate_book_id.expect("candidate_book_id set");
    let authors = svc.book_service.authors_for_book(book_id).await.unwrap();
    assert_eq!(authors.len(), 1, "duplicate author entries should be linked once");
}

#[tokio::test]
async fn process_job_dedupes_case_variant_genres() {
    // Open Library subjects routinely include case-variant duplicates
    // ("Fiction" and "FICTION", "Fantasy fiction" and "Fantasy Fiction").
    // GenreRepository:: find_by_name is case-insensitive, so both variants
    // resolve to one genre.id and without dedup the second add_book_genre
    // violates (book_id, genre_id).
    let metadata = ExtractedMetadata {
        title: Some("The Test Book".to_string()),
        authors: Some(vec![ExtractedAuthor {
            name: "Test Author".to_string(),
            role: Some(AuthorRole::Author),
            sort_order: 0,
        }]),
        genres: vec!["Fiction".to_string(), "FICTION".to_string()],
        ..Default::default()
    };
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, metadata);
    let job = fixtures::insert_import_job(&ctx.repos, "hash_duplicate_genres").await;

    let updated = svc
        .pipeline_service
        .process_job(job)
        .await
        .expect("process_job must not fail on case-variant genres");

    let book_id = updated.candidate_book_id.expect("candidate_book_id set");
    let genres = svc.book_service.genres_for_book(book_id).await.unwrap();
    assert_eq!(genres.len(), 1, "case-variant genres should be linked once");
}

#[tokio::test]
async fn process_job_skips_non_pending_job() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_skip_non_pending").await;
    let job = fixtures::set_job_status(&ctx.repos, job, ImportStatus::NeedsReview).await;
    let original_id = job.id;

    let returned = svc.pipeline_service.process_job(job).await.unwrap();

    // Must come back unchanged — no new book created
    assert_eq!(returned.id, original_id);
    assert_eq!(returned.status, ImportStatus::NeedsReview);
}

#[tokio::test]
async fn process_job_deduplicates_existing_file() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());

    // First import — succeeds and writes a book_file with this hash
    let job1 = fixtures::insert_import_job(&ctx.repos, "hash_dup").await;
    let updated1 = svc.pipeline_service.process_job(job1).await.unwrap();
    assert_eq!(updated1.status, ImportStatus::NeedsReview);

    // Second import of the same file hash — must be rejected as a duplicate
    let job2 = fixtures::insert_import_job(&ctx.repos, "hash_dup").await;
    let updated2 = svc.pipeline_service.process_job(job2).await.unwrap();
    assert_eq!(updated2.status, ImportStatus::Rejected);
    assert!(updated2.error_message.as_deref().unwrap_or("").contains("already exists"));
}

#[tokio::test]
async fn process_job_uses_filename_as_fallback_title() {
    let ctx = setup().await;
    // Extractor returns no title — pipeline should fall back to the filename
    // stem
    let svc = fixtures::pipeline_services(&ctx, ExtractedMetadata::default());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_no_title").await;

    let updated = svc.pipeline_service.process_job(job).await.unwrap();

    let book_id = updated.candidate_book_id.expect("candidate_book_id set");
    let book = fixtures::find_book_by_id(&ctx.repos, book_id).await.expect("book found");
    // file_path is "/watch/hash_no_title.epub" → stem is "hash_no_title"
    assert_eq!(book.title, "hash_no_title");
}

// ── reject_job tests
// ────────────────────────────────────────────────────────────

#[tokio::test]
async fn reject_job_removes_candidate_book() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_reject_book").await;
    let job = svc.pipeline_service.process_job(job).await.unwrap();
    let book_id = job.candidate_book_id.expect("candidate_book_id set");
    let book = fixtures::find_book_by_id(&ctx.repos, book_id).await.expect("book exists");

    svc.collection_service.reject_book(job.token).await.unwrap();

    let found = svc.book_service.find_book_by_token(book.token).await.unwrap();
    assert!(found.is_none(), "book should be deleted after reject");
}

#[tokio::test]
async fn reject_job_removes_import_job() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_reject_job").await;
    let job = svc.pipeline_service.process_job(job).await.unwrap();
    let job_id = job.id;

    svc.collection_service.reject_book(job.token).await.unwrap();

    let found = fixtures::find_job_by_id(&ctx.repos, job_id).await;
    assert!(found.is_none(), "import job should be deleted after reject");
}

#[tokio::test]
async fn reject_job_removes_orphan_author() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_reject_orphan").await;
    let job = svc.pipeline_service.process_job(job).await.unwrap();

    svc.collection_service.reject_book(job.token).await.unwrap();

    // "Test Author" was only on this book — must be cleaned up
    let books = svc.book_service.list_books(&BookQuery::default(), None, None, None).await.unwrap();
    assert!(books.is_empty(), "all books deleted");
    // No author listing service exposed directly; verify indirectly via library
    // stats
    let stats = svc.collection_service.collection_stats().await.unwrap();
    assert_eq!(stats.authors, 0, "orphan author should be removed");
}

#[tokio::test]
async fn reject_job_fails_when_not_needs_review() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_reject_bad_status").await;
    // Job is still Pending — reject must fail

    let result = svc.collection_service.reject_book(job.token).await;

    assert!(matches!(result, Err(Error::Validation(_))), "expected Validation error, got: {result:?}");
}

#[tokio::test]
async fn reject_job_fails_for_unknown_token() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let ghost_token = bb_core::import::ImportJobToken::new(999_999);

    let result = svc.collection_service.reject_book(ghost_token).await;

    assert!(
        matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))),
        "expected NotFound, got: {result:?}"
    );
}

// ── approve_job tests
// ───────────────────────────────────────────────────────────

#[tokio::test]
async fn approve_job_transitions_book_to_available() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_approve_available").await;
    let job = svc.pipeline_service.process_job(job).await.unwrap();
    let book_id = job.candidate_book_id.expect("candidate_book_id set");
    let book = fixtures::find_book_by_id(&ctx.repos, book_id).await.expect("book exists");
    let book_token = book.token;
    let edit = minimal_edit("Approved Book", "Test Author");
    let reviewer = fixtures::insert_user(&ctx.repos, "reviewer_approve").await;

    svc.collection_service
        .approve_book(job.token, reviewer.id, edit, &std::env::temp_dir())
        .await
        .unwrap();

    let book = svc.book_service.find_book_by_token(book_token).await.unwrap().expect("book found");
    assert_eq!(book.status, BookStatus::Available);
    assert_eq!(book.title, "Approved Book");
}

#[tokio::test]
async fn approve_job_fails_when_not_needs_review() {
    let ctx = setup().await;
    let svc = fixtures::pipeline_services(&ctx, stub_metadata());
    let job = fixtures::insert_import_job(&ctx.repos, "hash_approve_bad_status").await;
    // Job is still Pending — approve must fail
    let edit = minimal_edit("Title", "Author");

    let result = svc.collection_service.approve_book(job.token, 1, edit, &std::env::temp_dir()).await;

    assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound) | Error::Validation(_))));
}
