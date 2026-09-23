//! Shared Kobo protocol types and builder helpers.
//!
//! These types are used by both the library sync endpoint (M8.5) and the
//! per-book metadata endpoint (M8.8). All field names and values have been
//! validated against Komga (`BookEntitlementDto.kt`, `KoboBookMetadataDto.kt`,
//! `DownloadUrlDto.kt`) and Calibre-Web (`kobo.py`).

use std::collections::HashMap;

use bb_core::{
    book::{Book, BookFile, BookId, BookToken, FileFormat, FileRole},
    device::BookSyncEntry,
    reading::UserBookMetadata,
};
use serde::Serialize;

// ── Dummy constant
// ─────────────────────────────────────────────────────────────────

/// Placeholder UUID used for `Categories` and `Genre` when no real value
/// exists. Both Komga and Calibre-Web use the same sentinel.
pub(super) const DUMMY_ID: &str = "00000000-0000-0000-0000-000000000001";

// ── Wire types
// ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboActivePeriod {
    pub from: String,
}

/// Per-book entitlement. `IsRemoved: true` signals deletion to the Kobo.
///
/// Field names and values validated against Komga `BookEntitlementDto.kt` and
/// Calibre-Web `kobo.py :: create_book_entitlement()`.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboBookEntitlement {
    /// Always `"Full"` — string, not integer.
    pub accessibility: &'static str,
    pub active_period: KoboActivePeriod,
    pub created: String,
    pub cross_revision_id: String,
    pub id: String,
    pub is_hidden_from_archive: bool,
    pub is_locked: bool,
    pub is_removed: bool,
    pub last_modified: String,
    pub origin_category: &'static str,
    pub revision_id: String,
    /// Always `"Active"` — string, not integer.
    pub status: &'static str,
}

/// One entry in the `DownloadUrls` array.
///
/// The Kobo protocol requires an *array* of download URL objects, not a single
/// `DownloadUrl` string. Validated against Komga `DownloadUrlDto.kt` and
/// Calibre-Web `kobo.py`.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboDownloadUrl {
    pub drm_type: &'static str,
    pub format: &'static str,
    pub size: i64,
    pub platform: &'static str,
    pub url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboPrice {
    pub currency_code: &'static str,
    pub total_amount: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboPublisher {
    pub imprint: String,
    pub name: String,
}

/// Series metadata sent to the Kobo device.
///
/// Field names validated against Komga (`KoboSeriesDto.kt`) and
/// Calibre-Web (`kobo.py :: get_series_metadata()`).
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboSeries {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_float: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboBookMetadata {
    pub categories: Vec<&'static str>,
    pub contributor_roles: Vec<String>,
    pub contributors: Vec<String>,
    pub cover_image_id: String,
    pub cross_revision_id: String,
    pub current_display_price: KoboPrice,
    pub current_love_display_price: KoboPrice,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub download_urls: Vec<KoboDownloadUrl>,
    pub entitlement_id: String,
    pub external_ids: Vec<String>,
    pub genre: &'static str,
    pub is_eligible_for_kobo_love: bool,
    pub is_internet_archive: bool,
    pub is_pre_order: bool,
    pub is_social_enabled: bool,
    pub language: String,
    pub phonetic_pronunciations: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<KoboPublisher>,
    pub revision_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<KoboSeries>,
    pub title: String,
    pub work_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct KoboEntitlementContainer {
    pub book_entitlement: KoboBookEntitlement,
    pub book_metadata: KoboBookMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_state: Option<serde_json::Value>,
}

/// Top-level item in the library sync response array.
///
/// Serde's default (externally tagged) representation emits
/// `{"NewEntitlement": {...}}` or `{"ChangedEntitlement": {...}}`.
///
/// Removed books use `ChangedEntitlement` with `IsRemoved: true` — **not** a
/// `DeletedEntitlement` key (which does not exist in the Kobo protocol).
#[derive(Serialize)]
pub(super) enum KoboSyncItem {
    NewEntitlement(KoboEntitlementContainer),
    ChangedEntitlement(KoboEntitlementContainer),
}

// ── Helpers
// ─────────────────────────────────────────────────────────────────

/// Returns the Kobo-facing ID for a book: the encoded portion of the token
/// without the `BK_` prefix.
pub(super) fn book_uuid_from_token(token: BookToken) -> String {
    token.encoded_id()
}

pub(super) fn book_uuid_from_id(id: BookId) -> String {
    BookToken::new(id).encoded_id()
}

pub(super) fn build_entitlement(uuid: &str, is_removed: bool, created: &str, last_modified: &str) -> KoboBookEntitlement {
    KoboBookEntitlement {
        accessibility: "Full",
        active_period: KoboActivePeriod { from: created.to_string() },
        created: created.to_string(),
        cross_revision_id: uuid.to_string(),
        id: uuid.to_string(),
        is_hidden_from_archive: false,
        is_locked: false,
        is_removed,
        last_modified: last_modified.to_string(),
        origin_category: "Imported",
        revision_id: uuid.to_string(),
        status: "Active",
    }
}

/// Selects the best file to serve for a given book, following the same
/// priority order as `DeviceServiceImpl::select_best_file`:
/// Enriched Kepub → Enriched Epub → Original Kepub → Original Epub.
pub(super) fn select_best_file(files: &[BookFile]) -> Option<&BookFile> {
    let priority = [
        (FileFormat::Kepub, FileRole::Enriched),
        (FileFormat::Epub, FileRole::Enriched),
        (FileFormat::Kepub, FileRole::Original),
        (FileFormat::Epub, FileRole::Original),
    ];
    for (fmt, role) in &priority {
        if let Some(f) = files.iter().find(|f| &f.format == fmt && &f.file_role == role) {
            return Some(f);
        }
    }
    None
}

/// Builds the `KoboBookMetadata` for a book, optionally with a download URL
/// if a best file is provided. Used by both the library sync and per-book
/// metadata endpoints.
pub(super) fn build_book_metadata(book: &Book, file: Option<&BookFile>, sync_token: &str, base: &str, series_name: Option<String>) -> KoboBookMetadata {
    let uuid = book_uuid_from_token(book.token);

    let download_urls = if let Some(file) = file {
        let (format_str, kobo_format) = match file.format {
            FileFormat::Kepub => ("kepub", "KEPUB"),
            _ => ("epub", "EPUB3"),
        };
        let url = format!("{base}/kobo/{sync_token}/v1/download/{uuid}/{format_str}");
        vec![KoboDownloadUrl {
            drm_type: "None",
            format: kobo_format,
            size: file.file_size,
            platform: "Generic",
            url,
        }]
    } else {
        Vec::new()
    };

    KoboBookMetadata {
        categories: vec![DUMMY_ID],
        contributor_roles: Vec::new(),
        contributors: Vec::new(),
        cover_image_id: uuid.clone(),
        cross_revision_id: uuid.clone(),
        current_display_price: KoboPrice {
            currency_code: "USD",
            total_amount: 0,
        },
        current_love_display_price: KoboPrice {
            currency_code: "USD",
            total_amount: 0,
        },
        description: book.description.clone().filter(|s| !s.is_empty()),
        download_urls,
        entitlement_id: uuid.clone(),
        external_ids: Vec::new(),
        genre: DUMMY_ID,
        is_eligible_for_kobo_love: false,
        is_internet_archive: false,
        is_pre_order: false,
        is_social_enabled: true,
        language: book.language.clone().unwrap_or_else(|| "en".to_string()),
        phonetic_pronunciations: HashMap::new(),
        publication_date: book.published_date.map(|y| format!("{y}-01-01T00:00:00Z")),
        publisher: None,
        revision_id: uuid.clone(),
        series: series_name.map(|name| {
            let number_f64 = book.series_number.as_ref().and_then(|n| n.to_string().parse::<f64>().ok());
            KoboSeries {
                id: book.series_id.map(|id| id.to_string()).unwrap_or_default(),
                name,
                number: number_f64,
                number_float: number_f64,
            }
        }),
        title: book.title.clone(),
        work_id: uuid,
    }
}

pub(super) fn build_new_entitlement(
    entry: &BookSyncEntry,
    sync_token: &str,
    base: &str,
    reading_state: Option<&UserBookMetadata>,
    series_name: Option<String>,
) -> KoboSyncItem {
    let book = &entry.book;
    let uuid = book_uuid_from_token(book.token);
    let created = book.created_at.to_rfc3339();
    let last_modified = book.updated_at.to_rfc3339();

    let entitlement = build_entitlement(&uuid, false, &created, &last_modified);
    let metadata = build_book_metadata(book, Some(&entry.file), sync_token, base, series_name);

    KoboSyncItem::NewEntitlement(KoboEntitlementContainer {
        book_entitlement: entitlement,
        book_metadata: metadata,
        reading_state: reading_state.map(|rs| super::library_state::build_kobo_state(book, rs)),
    })
}

/// Builds a `ChangedEntitlement` for a book that the device already has but
/// whose file or metadata has changed (upgrade to KEPUB, metadata edit, etc.).
pub(super) fn build_changed_entitlement(
    entry: &BookSyncEntry,
    sync_token: &str,
    base: &str,
    reading_state: Option<&UserBookMetadata>,
    series_name: Option<String>,
) -> KoboSyncItem {
    let book = &entry.book;
    let uuid = book_uuid_from_token(book.token);
    let created = book.created_at.to_rfc3339();
    let last_modified = book.updated_at.to_rfc3339();

    let entitlement = build_entitlement(&uuid, false, &created, &last_modified);
    let metadata = build_book_metadata(book, Some(&entry.file), sync_token, base, series_name);

    KoboSyncItem::ChangedEntitlement(KoboEntitlementContainer {
        book_entitlement: entitlement,
        book_metadata: metadata,
        reading_state: reading_state.map(|rs| super::library_state::build_kobo_state(book, rs)),
    })
}

pub(super) fn build_removed_entitlement(book_id: BookId) -> KoboSyncItem {
    use chrono::Utc;
    let uuid = book_uuid_from_id(book_id);
    let now = Utc::now().to_rfc3339();

    let entitlement = build_entitlement(&uuid, true, &now, &now);

    // Minimal metadata — the Kobo only needs the ID fields to identify which
    // book to remove. Title is required by the schema so we use an empty
    // string.
    let metadata = KoboBookMetadata {
        categories: vec![DUMMY_ID],
        contributor_roles: Vec::new(),
        contributors: Vec::new(),
        cover_image_id: uuid.clone(),
        cross_revision_id: uuid.clone(),
        current_display_price: KoboPrice {
            currency_code: "USD",
            total_amount: 0,
        },
        current_love_display_price: KoboPrice {
            currency_code: "USD",
            total_amount: 0,
        },
        description: None,
        download_urls: Vec::new(),
        entitlement_id: uuid.clone(),
        external_ids: Vec::new(),
        genre: DUMMY_ID,
        is_eligible_for_kobo_love: false,
        is_internet_archive: false,
        is_pre_order: false,
        is_social_enabled: true,
        language: "en".to_string(),
        phonetic_pronunciations: HashMap::new(),
        publication_date: None,
        publisher: None,
        revision_id: uuid.clone(),
        series: None,
        title: String::new(),
        work_id: uuid,
    };

    KoboSyncItem::ChangedEntitlement(KoboEntitlementContainer {
        book_entitlement: entitlement,
        book_metadata: metadata,
        reading_state: None,
    })
}
