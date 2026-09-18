use bb_core::{
    RepositoryError,
    filter::{ACTIVE_STATUSES, BookFilter, DateOp, EntityRef, FilterCondition, FilterReadStatus, FilterRule, NumericOp, SetOp, TextOp},
    user::UserId,
};
use chrono::DateTime;
use sea_orm::{
    ColumnTrait, Condition, ExprTrait,
    sea_query::{BinOper, Expr, Func, Query},
};

use crate::entities::{authors, book_authors, book_genres, book_shelves, book_tags, books, library_books, user_book_metadata};

// ── Public entry point
// ────────────────────────────────────────────────────────

/// Recursively translate a [`BookFilter`] tree into a SeaORM [`Condition`].
///
/// The resulting condition can be passed directly to `.filter()` on any SeaORM
/// query that operates on the `books` table.  All relation-based rules use
/// subqueries so no extra JOINs are required on the outer query.
///
/// Returns `Err` if the filter contains a user-scoped rule (e.g. `ReadStatus`)
/// and `user_id` is `0` (the sentinel for "no user context").
pub(crate) fn build_condition(filter: &BookFilter, user_id: UserId) -> Result<Condition, RepositoryError> {
    match filter {
        BookFilter::Group(group) => {
            let base = match group.condition {
                FilterCondition::And => Condition::all(),
                FilterCondition::Or => Condition::any(),
            };
            group.items.iter().try_fold(base, |acc, item| Ok(acc.add(build_condition(item, user_id)?)))
        }
        BookFilter::Rule(rule) => rule_condition(rule, user_id),
    }
}

// ── Rule dispatch
// ─────────────────────────────────────────────────────────────

fn rule_condition(rule: &FilterRule, user_id: UserId) -> Result<Condition, RepositoryError> {
    match rule {
        FilterRule::TitleText { op, value } => Ok(title_text_condition(*op, value)),
        FilterRule::AuthorText { op, value } => Ok(author_text_condition(*op, value)),
        FilterRule::Author { op, values } => Ok(author_condition(*op, values)),
        FilterRule::Series { op, values } => Ok(series_condition(*op, values)),
        FilterRule::Genre { op, values } => Ok(genre_condition(*op, values)),
        FilterRule::Tag { op, values } => Ok(tag_condition(*op, values)),
        FilterRule::Publisher { op, values } => Ok(publisher_condition(*op, values)),
        FilterRule::Shelf { op, values } => Ok(shelf_condition(*op, values)),
        FilterRule::Library { op, values } => Ok(library_condition(*op, values)),
        FilterRule::Language { op, values } => Ok(language_condition(*op, values)),
        FilterRule::ReadStatus { op, values } => read_status_condition(*op, values, user_id),
        FilterRule::Rating { op, value } => Ok(rating_condition(*op, *value)),
        FilterRule::DateAdded { op, value } => Ok(date_added_condition(*op, *value)),
    }
}

// ── Text rules
// ────────────────────────────────────────────────────────────────

fn title_text_condition(op: TextOp, value: &str) -> Condition {
    let lower_col = Expr::expr(Func::lower(Expr::col(books::Column::Title)));
    match op {
        TextOp::Contains => Condition::all().add(lower_col.binary(BinOper::Like, Expr::value(format!("%{}%", value.to_lowercase())))),
        TextOp::DoesntContain => Condition::all().add(lower_col.binary(BinOper::NotLike, Expr::value(format!("%{}%", value.to_lowercase())))),
        TextOp::StartsWith => Condition::all().add(lower_col.binary(BinOper::Like, Expr::value(format!("{}%", value.to_lowercase())))),
        TextOp::EndsWith => Condition::all().add(lower_col.binary(BinOper::Like, Expr::value(format!("%{}", value.to_lowercase())))),
        TextOp::Equals => Condition::all().add(lower_col.binary(BinOper::Equal, Expr::value(value.to_lowercase()))),
        TextOp::NotEquals => Condition::all().add(lower_col.binary(BinOper::NotEqual, Expr::value(value.to_lowercase()))),
        TextOp::IsEmpty => Condition::all().add(books::Column::Title.eq("")),
        TextOp::IsNotEmpty => Condition::all().add(books::Column::Title.ne("")),
    }
}

/// Books that have at least one author whose name matches the text rule.
fn author_text_condition(op: TextOp, value: &str) -> Condition {
    // Inner subquery: author IDs matching the name condition
    let author_ids_subq = |bin_op: BinOper, pattern: String| {
        let mut q = Query::select();
        q.column(authors::Column::Id)
            .from(authors::Entity)
            .and_where(Expr::expr(Func::lower(Expr::col(authors::Column::Name))).binary(bin_op, Expr::value(pattern)));
        q
    };

    // Outer subquery: book IDs that have an author from the inner subquery
    let book_ids_with_author = |author_subq| {
        let mut q = Query::select();
        q.column(book_authors::Column::BookId)
            .from(book_authors::Entity)
            .and_where(book_authors::Column::AuthorId.in_subquery(author_subq));
        q
    };

    // All book IDs that have any author
    let any_author_subq = || {
        let mut q = Query::select();
        q.column(book_authors::Column::BookId).from(book_authors::Entity);
        q
    };

    match op {
        TextOp::Contains => {
            let inner = author_ids_subq(BinOper::Like, format!("%{}%", value.to_lowercase()));
            Condition::all().add(books::Column::Id.in_subquery(book_ids_with_author(inner)))
        }
        TextOp::DoesntContain => {
            let inner = author_ids_subq(BinOper::Like, format!("%{}%", value.to_lowercase()));
            Condition::all().add(books::Column::Id.not_in_subquery(book_ids_with_author(inner)))
        }
        TextOp::StartsWith => {
            let inner = author_ids_subq(BinOper::Like, format!("{}%", value.to_lowercase()));
            Condition::all().add(books::Column::Id.in_subquery(book_ids_with_author(inner)))
        }
        TextOp::EndsWith => {
            let inner = author_ids_subq(BinOper::Like, format!("%{}", value.to_lowercase()));
            Condition::all().add(books::Column::Id.in_subquery(book_ids_with_author(inner)))
        }
        TextOp::Equals => {
            let inner = author_ids_subq(BinOper::Equal, value.to_lowercase());
            Condition::all().add(books::Column::Id.in_subquery(book_ids_with_author(inner)))
        }
        TextOp::NotEquals => {
            let inner = author_ids_subq(BinOper::Equal, value.to_lowercase());
            Condition::all().add(books::Column::Id.not_in_subquery(book_ids_with_author(inner)))
        }
        TextOp::IsEmpty => {
            // Books with no authors
            Condition::all().add(books::Column::Id.not_in_subquery(any_author_subq()))
        }
        TextOp::IsNotEmpty => {
            // Books with at least one author
            Condition::all().add(books::Column::Id.in_subquery(any_author_subq()))
        }
    }
}

// ── Junction table rules (Author, Genre, Tag)
// ─────────────────────────────────

fn author_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();

    // SELECT book_id FROM book_authors WHERE author_id IN (ids)
    let any_subq = |ids: Vec<i64>| {
        let mut q = Query::select();
        q.column(book_authors::Column::BookId)
            .from(book_authors::Entity)
            .and_where(book_authors::Column::AuthorId.is_in(ids));
        q
    };

    // SELECT book_id FROM book_authors WHERE author_id = id
    let one_subq = |id: i64| {
        let mut q = Query::select();
        q.column(book_authors::Column::BookId)
            .from(book_authors::Entity)
            .and_where(book_authors::Column::AuthorId.eq(id));
        q
    };

    // SELECT book_id FROM book_authors (all associations)
    let all_subq = || {
        let mut q = Query::select();
        q.column(book_authors::Column::BookId).from(book_authors::Entity);
        q
    };

    junction_set_condition(op, ids, any_subq, one_subq, all_subq)
}

fn genre_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();

    let any_subq = |ids: Vec<i64>| {
        let mut q = Query::select();
        q.column(book_genres::Column::BookId)
            .from(book_genres::Entity)
            .and_where(book_genres::Column::GenreId.is_in(ids));
        q
    };

    let one_subq = |id: i64| {
        let mut q = Query::select();
        q.column(book_genres::Column::BookId)
            .from(book_genres::Entity)
            .and_where(book_genres::Column::GenreId.eq(id));
        q
    };

    let all_subq = || {
        let mut q = Query::select();
        q.column(book_genres::Column::BookId).from(book_genres::Entity);
        q
    };

    junction_set_condition(op, ids, any_subq, one_subq, all_subq)
}

fn tag_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();

    let any_subq = |ids: Vec<i64>| {
        let mut q = Query::select();
        q.column(book_tags::Column::BookId)
            .from(book_tags::Entity)
            .and_where(book_tags::Column::TagId.is_in(ids));
        q
    };

    let one_subq = |id: i64| {
        let mut q = Query::select();
        q.column(book_tags::Column::BookId)
            .from(book_tags::Entity)
            .and_where(book_tags::Column::TagId.eq(id));
        q
    };

    let all_subq = || {
        let mut q = Query::select();
        q.column(book_tags::Column::BookId).from(book_tags::Entity);
        q
    };

    junction_set_condition(op, ids, any_subq, one_subq, all_subq)
}

fn shelf_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();

    let any_subq = |ids: Vec<i64>| {
        let mut q = Query::select();
        q.column(book_shelves::Column::BookId)
            .from(book_shelves::Entity)
            .and_where(book_shelves::Column::ShelfId.is_in(ids));
        q
    };

    let one_subq = |id: i64| {
        let mut q = Query::select();
        q.column(book_shelves::Column::BookId)
            .from(book_shelves::Entity)
            .and_where(book_shelves::Column::ShelfId.eq(id));
        q
    };

    let all_subq = || {
        let mut q = Query::select();
        q.column(book_shelves::Column::BookId).from(book_shelves::Entity);
        q
    };

    junction_set_condition(op, ids, any_subq, one_subq, all_subq)
}

fn library_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();

    let any_subq = |ids: Vec<i64>| {
        let mut q = Query::select();
        q.column(library_books::Column::BookId)
            .from(library_books::Entity)
            .and_where(library_books::Column::LibraryId.is_in(ids));
        q
    };

    let one_subq = |id: i64| {
        let mut q = Query::select();
        q.column(library_books::Column::BookId)
            .from(library_books::Entity)
            .and_where(library_books::Column::LibraryId.eq(id));
        q
    };

    let all_subq = || {
        let mut q = Query::select();
        q.column(library_books::Column::BookId).from(library_books::Entity);
        q
    };

    junction_set_condition(op, ids, any_subq, one_subq, all_subq)
}

/// Shared SetOp logic for junction-table rules (Author, Genre, Tag).
///
/// - `any_subq(ids)` — returns `SELECT book_id FROM junction WHERE entity_id IN
///   (ids)`
/// - `one_subq(id)`  — returns `SELECT book_id FROM junction WHERE entity_id =
///   id`
/// - `all_subq()`    — returns `SELECT book_id FROM junction` (all
///   associations)
fn junction_set_condition(
    op: SetOp,
    ids: Vec<i64>,
    any_subq: impl Fn(Vec<i64>) -> sea_orm::sea_query::SelectStatement,
    one_subq: impl Fn(i64) -> sea_orm::sea_query::SelectStatement,
    all_subq: impl Fn() -> sea_orm::sea_query::SelectStatement,
) -> Condition {
    match op {
        SetOp::IncludesAny => {
            if ids.is_empty() {
                return never();
            }
            Condition::all().add(books::Column::Id.in_subquery(any_subq(ids)))
        }
        SetOp::IncludesAll => {
            if ids.is_empty() {
                return never();
            }
            ids.into_iter()
                .fold(Condition::all(), |cond, id| cond.add(books::Column::Id.in_subquery(one_subq(id))))
        }
        SetOp::ExcludesAll => {
            if ids.is_empty() {
                return Condition::all();
            }
            Condition::all().add(books::Column::Id.not_in_subquery(any_subq(ids)))
        }
        SetOp::IsEmpty => Condition::all().add(books::Column::Id.not_in_subquery(all_subq())),
        SetOp::IsNotEmpty => Condition::all().add(books::Column::Id.in_subquery(all_subq())),
    }
}

// ── Direct FK rules (Series, Publisher) ──────────────────────────────────────

fn series_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();
    // Series is a single-value FK; IncludesAll degrades to IncludesAny.
    match op {
        SetOp::IncludesAny | SetOp::IncludesAll => {
            if ids.is_empty() {
                return never();
            }
            Condition::all().add(books::Column::SeriesId.is_in(ids))
        }
        SetOp::ExcludesAll => {
            if ids.is_empty() {
                return Condition::all();
            }
            // Books with a different series OR no series at all
            Condition::any()
                .add(books::Column::SeriesId.is_not_in(ids))
                .add(books::Column::SeriesId.is_null())
        }
        SetOp::IsEmpty => Condition::all().add(books::Column::SeriesId.is_null()),
        SetOp::IsNotEmpty => Condition::all().add(books::Column::SeriesId.is_not_null()),
    }
}

fn publisher_condition(op: SetOp, values: &[EntityRef]) -> Condition {
    let ids: Vec<i64> = values.iter().map(|e| e.id).collect();
    // Publisher is a single-value FK; IncludesAll degrades to IncludesAny.
    match op {
        SetOp::IncludesAny | SetOp::IncludesAll => {
            if ids.is_empty() {
                return never();
            }
            Condition::all().add(books::Column::PublisherId.is_in(ids))
        }
        SetOp::ExcludesAll => {
            if ids.is_empty() {
                return Condition::all();
            }
            Condition::any()
                .add(books::Column::PublisherId.is_not_in(ids))
                .add(books::Column::PublisherId.is_null())
        }
        SetOp::IsEmpty => Condition::all().add(books::Column::PublisherId.is_null()),
        SetOp::IsNotEmpty => Condition::all().add(books::Column::PublisherId.is_not_null()),
    }
}

// ── Language rule (string set on a nullable column)
// ───────────────────────────

fn language_condition(op: SetOp, values: &[String]) -> Condition {
    let vals: Vec<String> = values.to_vec();
    match op {
        SetOp::IncludesAny | SetOp::IncludesAll => {
            if vals.is_empty() {
                return never();
            }
            Condition::all().add(books::Column::Language.is_in(vals))
        }
        SetOp::ExcludesAll => {
            if vals.is_empty() {
                return Condition::all();
            }
            Condition::any()
                .add(books::Column::Language.is_not_in(vals))
                .add(books::Column::Language.is_null())
        }
        SetOp::IsEmpty => Condition::all().add(books::Column::Language.is_null()),
        SetOp::IsNotEmpty => Condition::all().add(books::Column::Language.is_not_null()),
    }
}

// ── ReadStatus rule
// ───────────────────────────────────────────────────────────

fn read_status_condition(op: SetOp, values: &[FilterReadStatus], user_id: UserId) -> Result<Condition, RepositoryError> {
    if user_id == 0 {
        return Err(RepositoryError::Constraint(
            "ReadStatus filter requires a valid user context (user_id must not be 0)".to_string(),
        ));
    }
    let statuses = expand_read_statuses(values);
    let unread_included = statuses.contains(&"unread");

    // SELECT book_id FROM user_book_metadata WHERE user_id = ?
    let ubm_for_user = || {
        let mut q = Query::select();
        q.column(user_book_metadata::Column::BookId)
            .from(user_book_metadata::Entity)
            .and_where(user_book_metadata::Column::UserId.eq(user_id as i64));
        q
    };

    // SELECT book_id FROM user_book_metadata WHERE user_id = ? AND read_status
    // IN (statuses)
    let ubm_with_status = |statuses: Vec<&'static str>| {
        let mut q = Query::select();
        q.column(user_book_metadata::Column::BookId)
            .from(user_book_metadata::Entity)
            .and_where(user_book_metadata::Column::UserId.eq(user_id as i64))
            .and_where(user_book_metadata::Column::ReadStatus.is_in(statuses));
        q
    };

    Ok(match op {
        // IncludesAll degrades to IncludesAny: a book has exactly one read status.
        SetOp::IncludesAny | SetOp::IncludesAll => {
            if statuses.is_empty() {
                return Ok(never());
            }
            if unread_included {
                // No UBM row (implicit unread) OR UBM row with a matching
                // status
                Condition::any()
                    .add(books::Column::Id.not_in_subquery(ubm_for_user()))
                    .add(books::Column::Id.in_subquery(ubm_with_status(statuses)))
            } else {
                Condition::all().add(books::Column::Id.in_subquery(ubm_with_status(statuses)))
            }
        }
        SetOp::ExcludesAll => {
            if statuses.is_empty() {
                return Ok(Condition::all());
            }
            if unread_included {
                // A book with no UBM row is effectively "unread" → it IS in the
                // excluded set, so it must be filtered out.
                // Only books with an explicit UBM row
                // whose status is NOT in the excluded set pass through.
                Condition::all()
                    .add(books::Column::Id.in_subquery(ubm_for_user()))
                    .add(books::Column::Id.not_in_subquery(ubm_with_status(statuses)))
            } else {
                // Books with no UBM row have implicit status "unread" which is
                // NOT in the excluded set → include them.
                // Books with a matching UBM status → exclude.
                Condition::all().add(books::Column::Id.not_in_subquery(ubm_with_status(statuses)))
            }
        }
        // IsEmpty: no UBM row has ever been written for this book+user pair
        SetOp::IsEmpty => Condition::all().add(books::Column::Id.not_in_subquery(ubm_for_user())),
        // IsNotEmpty: at least one UBM row exists (user has interacted with the book)
        SetOp::IsNotEmpty => Condition::all().add(books::Column::Id.in_subquery(ubm_for_user())),
    })
}

/// Expand [`FilterReadStatus::Active`] to its constituent statuses and
/// deduplicate.  Returns string slices ready for SQL `IN` comparisons.
fn expand_read_statuses(values: &[FilterReadStatus]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for v in values {
        match v {
            FilterReadStatus::Active => {
                for s in ACTIVE_STATUSES {
                    out.push(filter_read_status_to_str(s));
                }
            }
            other => out.push(filter_read_status_to_str(other)),
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn filter_read_status_to_str(s: &FilterReadStatus) -> &'static str {
    match s {
        FilterReadStatus::Unread => "unread",
        FilterReadStatus::Reading => "reading",
        FilterReadStatus::Paused => "paused",
        FilterReadStatus::Rereading => "rereading",
        FilterReadStatus::Read => "read",
        FilterReadStatus::Abandoned => "abandoned",
        FilterReadStatus::Active => unreachable!("Active must be expanded before conversion"),
    }
}

// ── Numeric / date rules
// ──────────────────────────────────────────────────────

fn rating_condition(op: NumericOp, value: u8) -> Condition {
    let val = i16::from(value);
    let expr = match op {
        NumericOp::Eq => books::Column::Rating.eq(val),
        NumericOp::NotEq => books::Column::Rating.ne(val),
        NumericOp::Lt => books::Column::Rating.lt(val),
        NumericOp::Lte => books::Column::Rating.lte(val),
        NumericOp::Gt => books::Column::Rating.gt(val),
        NumericOp::Gte => books::Column::Rating.gte(val),
    };
    Condition::all().add(expr)
}

fn date_added_condition(op: DateOp, value: Option<DateTime<chrono::Utc>>) -> Condition {
    match op {
        DateOp::Before => match value {
            Some(dt) => Condition::all().add(books::Column::CreatedAt.lt(dt.fixed_offset())),
            None => Condition::all(),
        },
        DateOp::After => match value {
            Some(dt) => Condition::all().add(books::Column::CreatedAt.gt(dt.fixed_offset())),
            None => Condition::all(),
        },
        // created_at is never NULL in practice; these map to vacuous true/false
        DateOp::IsEmpty => Condition::all().add(books::Column::CreatedAt.is_null()),
        DateOp::IsNotEmpty => Condition::all().add(books::Column::CreatedAt.is_not_null()),
    }
}

// ── Helpers
// ───────────────────────────────────────────────────────────────────

/// A condition that matches nothing.  Used when a set rule is given an empty
/// value list (e.g. `Author IncludesAny []` — there are no valid books).
fn never() -> Condition {
    // 1 = 0 is always false and is portable across all supported backends.
    Condition::all().add(Expr::val(1i32).eq(0i32))
}
