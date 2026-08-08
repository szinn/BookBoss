use dioxus::prelude::*;

use crate::{
    Route,
    routes::{
        book_detail_page::delete_library_book,
        books_page::{BookSummary, remove_book_from_library},
        shelf_page::remove_book_from_shelf,
    },
};

// ---------------------------------------------------------------------------
// Context types
// ---------------------------------------------------------------------------

/// Describes the viewing context so `BookCard` knows what the × button does
/// and which author/series links to suppress (to avoid linking to the current
/// page).
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum BookGridContext {
    /// System library (All Books): × physically deletes the book. Requires
    /// `DeleteBook` capability (`can_delete`).
    SystemLibrary { can_delete: bool },
    /// Non-system library (personal or shared). `can_remove` is true when the
    /// user is the library owner OR holds `DeleteBook`. When true, × removes
    /// the book from the library (not a physical delete).
    NonSystemLibrary { library_token: String, can_remove: bool },
    /// Owner is viewing one of their own shelves: × removes the book from the
    /// shelf.
    OwnShelf { shelf_token: String },
    /// Read-only view (e.g. someone else's shelf, author/series pages): no ×
    /// button. Supply `current_author_token` or `current_series_token` to
    /// suppress the corresponding link when already on that page.
    ReadOnly {
        current_author_token: Option<String>,
        current_series_token: Option<String>,
    },
}

/// Page-level signal tracking the token of the book currently being dragged,
/// or `None` when no drag is in progress. Provided by pages, read by both
/// `BookCard` (writes on drag) and `ShelfBar` (reads for drop-target
/// highlight).
pub(crate) type DraggedBookToken = Signal<Option<String>>;

/// Global signal mirroring the page-level `DraggedBookToken`.
/// Written by `BookCard` alongside the page-local signal so that components
/// outside the page's context tree (e.g. `NavBar`) can also react to drags.
pub(crate) static DRAGGED_BOOK_GLOBAL: GlobalSignal<Option<String>> = Signal::global(|| None);

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn BookGrid(
    books: Vec<BookSummary>,
    context: BookGridContext,
    /// Called after any successful card action (delete / remove from shelf).
    /// The parent page should use this to restart its book-list resource.
    on_action: EventHandler<()>,
) -> Element {
    use_context_provider(|| context);
    use_context_provider(|| on_action);
    rsx! {
        div { class: "p-4",
            div { class: "grid gap-x-8 gap-y-4",
                style: "grid-template-columns: repeat(auto-fill, minmax(120px, 1fr))",
                for book in &books {
                    BookCard { book: book.clone() }
                }
            }
        }
    }
}

#[component]
fn BookCard(book: BookSummary) -> Element {
    let navigator = use_navigator();
    let token = book.token.clone();
    let ctx = use_context::<BookGridContext>();
    let on_action = use_context::<EventHandler<()>>();
    let mut dragged_token = use_context::<DraggedBookToken>();
    let mut show_confirm = use_signal(|| false);
    let mut deleting = use_signal(|| false);

    let in_selection_mode = super::selection::SELECTION_MODE();
    let selected = super::selection::is_selected(&book.token);

    let series_line = match (&book.series_name, &book.series_number) {
        (Some(name), Some(num)) => Some(format!("{name} #{num}")),
        (Some(name), None) => Some(name.clone()),
        _ => None,
    };

    let (suppressed_author_token, suppressed_series_token) = match &ctx {
        BookGridContext::ReadOnly {
            current_author_token,
            current_series_token,
        } => (current_author_token.clone(), current_series_token.clone()),
        _ => (None, None),
    };

    let is_dragging = !in_selection_mode && dragged_token().as_deref() == Some(book.token.as_str());

    // Whether the × triggers a physical delete (with confirmation).
    let is_library_delete = matches!(ctx, BookGridContext::SystemLibrary { can_delete: true });

    // Plain remove action (shelf or non-system library).
    let remove_action: Option<Box<dyn Fn() + 'static>> = match &ctx {
        BookGridContext::OwnShelf { shelf_token } => {
            let stok = shelf_token.clone();
            let btok = book.token.clone();
            Some(Box::new(move || {
                let s = stok.clone();
                let b = btok.clone();
                spawn(async move {
                    if remove_book_from_shelf(s, b).await.is_ok() {
                        on_action.call(());
                    }
                });
            }))
        }
        BookGridContext::NonSystemLibrary {
            library_token,
            can_remove: true,
        } => {
            let ltok = library_token.clone();
            let btok = book.token.clone();
            Some(Box::new(move || {
                let l = ltok.clone();
                let b = btok.clone();
                spawn(async move {
                    if remove_book_from_library(l, b).await.is_ok() {
                        on_action.call(());
                    }
                });
            }))
        }
        _ => None,
    };

    let show_x = !in_selection_mode && (is_library_delete || remove_action.is_some());

    rsx! {
        // Delete confirmation modal (library delete only)
        if show_confirm() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onkeydown: move |e| { if e.key() == Key::Escape { show_confirm.set(false); } },
                div { class: "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-sm mx-4",
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-2", "Delete Book?" }
                    p { class: "text-sm text-gray-600 dark:text-slate-400 mb-6",
                        "This will permanently delete \"{book.title}\" and all its files. This cannot be undone."
                    }
                    div { class: "flex gap-3 justify-end",
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                            autofocus: true,
                            onclick: move |_| show_confirm.set(false),
                            "No, Keep It"
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded bg-red-600 text-white hover:bg-red-700 disabled:opacity-50",
                            disabled: deleting(),
                            onclick: {
                                let tok = book.token.clone();
                                move |_| {
                                    let tok = tok.clone();
                                    deleting.set(true);
                                    show_confirm.set(false);
                                    spawn(async move {
                                        if delete_library_book(tok).await.is_ok() {
                                            on_action.call(());
                                        }
                                        deleting.set(false);
                                    });
                                }
                            },
                            if deleting() { "Deleting…" } else { "Yes, Delete" }
                        }
                    }
                }
            }
        }

        div {
            class: if is_dragging {
                "flex flex-col bg-white dark:bg-slate-800 opacity-50"
            } else if in_selection_mode && selected {
                "flex flex-col bg-white dark:bg-slate-800 ring-2 ring-indigo-500 rounded"
            } else {
                "flex flex-col bg-white dark:bg-slate-800"
            },
            draggable: !in_selection_mode,
            ondragstart: {
                let tok = book.token.clone();
                move |_| {
                    if !in_selection_mode {
                        *dragged_token.write() = Some(tok.clone());
                        *DRAGGED_BOOK_GLOBAL.write() = Some(tok.clone());
                    }
                }
            },
            ondragend: move |_| {
                *dragged_token.write() = None;
                *DRAGGED_BOOK_GLOBAL.write() = None;
            },

            div { class: "relative cursor-pointer",
                onclick: {
                    let tok = token.clone();
                    move |_| {
                        if in_selection_mode {
                            super::selection::toggle_selection(&tok);
                        } else {
                            navigator.push(Route::BookDetailPage { token: tok.clone() });
                        }
                    }
                },
                img {
                    src: "/api/v1/covers/{book.token}?v={book.updated_at}",
                    alt: "{book.title}",
                    class: "w-full max-w-[256px] mx-auto object-cover rounded shadow-sm",
                    style: "aspect-ratio: 2/3",
                }
                // Reading status badge
                if let Some(ref rs) = book.reading_state {
                    {
                        let (badge_class, badge_label) = match rs.status.as_str() {
                            "Reading" | "Rereading" => (
                                "absolute top-1 left-1 px-1 py-0.5 text-xs font-semibold rounded bg-indigo-600/85 text-white leading-none",
                                rs.status.as_str(),
                            ),
                            "Paused" => (
                                "absolute top-1 left-1 px-1 py-0.5 text-xs font-semibold rounded bg-yellow-500/85 text-white leading-none",
                                "Paused",
                            ),
                            "Read" => (
                                "absolute top-1 left-1 px-1 py-0.5 text-xs font-semibold rounded bg-green-600/85 text-white leading-none",
                                "✓ Read",
                            ),
                            "Abandoned" => (
                                "absolute top-1 left-1 px-1 py-0.5 text-xs font-semibold rounded bg-red-600/85 text-white leading-none",
                                "Abandoned",
                            ),
                            _ => ("", ""),
                        };
                        rsx! {
                            if !badge_label.is_empty() {
                                span { class: badge_class, { badge_label } }
                            }
                        }
                    }
                    // Progress bar at bottom when actively reading
                    if matches!(rs.status.as_str(), "Reading" | "Rereading" | "Paused") {
                        if let Some(pct) = rs.progress_pct {
                            if pct > 0 {
                                div {
                                    class: "absolute bottom-0 left-0 right-0 h-1 bg-black/20 rounded-b overflow-hidden",
                                    div { class: "h-full bg-indigo-400 dark:bg-indigo-500", style: "width: {pct}%" }
                                }
                            }
                        }
                    }
                }
                // Selection checkbox (top-right)
                if in_selection_mode {
                    div {
                        class: if selected {
                            "absolute top-1 right-1 w-5 h-5 rounded-full bg-indigo-600 flex items-center justify-center"
                        } else {
                            "absolute top-1 right-1 w-5 h-5 rounded-full border-2 border-white/80 bg-black/20"
                        },
                        if selected {
                            svg {
                                class: "w-3 h-3 text-white",
                                xmlns: "http://www.w3.org/2000/svg",
                                view_box: "0 0 20 20",
                                fill: "currentColor",
                                path {
                                    fill_rule: "evenodd",
                                    d: "M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z",
                                    clip_rule: "evenodd",
                                }
                            }
                        }
                    }
                }
                if show_x {
                    button {
                        class: "absolute top-1 right-1 w-5 h-5 flex items-center justify-center rounded-full bg-black/50 text-white text-xs hover:bg-red-600/80 leading-none",
                        title: if is_library_delete { "Delete" } else { "Remove" },
                        onclick: move |e| {
                            e.stop_propagation();
                            if is_library_delete {
                                show_confirm.set(true);
                            } else if let Some(ref action) = remove_action {
                                action();
                            }
                        },
                        "×"
                    }
                }
            }
            div { class: "mt-1 px-0.5",
                p { class: "text-xs font-semibold text-gray-900 dark:text-slate-100 leading-tight line-clamp-2",
                    "{book.title}"
                }
                p { class: "text-xs text-gray-500 dark:text-slate-400 leading-tight truncate mt-0.5",
                    for (i, author) in book.authors.iter().enumerate() {
                        if i > 0 {
                            span { ", " }
                        }
                        if suppressed_author_token.as_deref() == Some(author.token.as_str()) {
                            span { "{author.name}" }
                        } else {
                            Link {
                                to: Route::AuthorDetailPage { token: author.token.clone() },
                                class: "hover:underline",
                                "{author.name}"
                            }
                        }
                    }
                }
                if let Some(series) = series_line {
                    p { class: "text-xs text-gray-400 dark:text-slate-500 leading-tight truncate mt-0.5",
                        if let Some(ref stok) = book.series_token {
                            if suppressed_series_token.as_deref() == Some(stok.as_str()) {
                                span { {series} }
                            } else {
                                Link {
                                    to: Route::SeriesDetailPage { token: stok.clone() },
                                    class: "hover:underline",
                                    {series}
                                }
                            }
                        } else {
                            span { {series} }
                        }
                    }
                }
            }
        }
    }
}
