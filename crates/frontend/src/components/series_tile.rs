use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::Route;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SeriesTileData {
    pub token: String,
    pub name: String,
    pub book_count: u64,
    pub first_author_token: Option<String>,
    pub first_author_name: Option<String>,
    /// Book tokens for cover images (up to 3, earliest first).
    pub cover_book_tokens: Vec<String>,
}

/// A card showing fanned cover art for a series with name, author, and count.
#[component]
pub(crate) fn SeriesTile(data: SeriesTileData) -> Element {
    let covers = &data.cover_book_tokens;

    rsx! {
        div { class: "flex flex-col items-center",
            // Fanned cover art container — extra width for rotated side covers
            Link {
                to: Route::SeriesDetailPage { token: data.token.clone() },
                class: "relative w-[160px] h-[180px] block cursor-pointer",

                if covers.is_empty() {
                    // Placeholder when no covers
                    div {
                        class: "absolute rounded shadow-sm bg-gray-200 flex items-center justify-center",
                        style: "width: 120px; height: 180px; left: 20px",
                        span { class: "text-gray-400 text-xs", "No cover" }
                    }
                } else {
                    // Back covers rendered first (lower z-index), front cover last (on top)
                    for (i, book_token) in covers.iter().enumerate().rev() {
                        {
                            // Cover 0: front, centered. Cover 1: behind-left, rotated CCW. Cover 2: behind-right, rotated CW.
                            let style = match i {
                                0 => "position: absolute; width: 120px; height: 180px; left: 20px; top: 0; z-index: 30".to_string(),
                                1 => "position: absolute; width: 110px; height: 165px; left: 0; top: 8px; z-index: 10; transform: rotate(-8deg)".to_string(),
                                _ => "position: absolute; width: 110px; height: 165px; right: 0; top: 8px; z-index: 10; transform: rotate(8deg)".to_string(),
                            };
                            rsx! {
                                img {
                                    key: "{book_token}",
                                    src: "/api/v1/covers/{book_token}",
                                    alt: "{data.name}",
                                    class: "object-cover rounded shadow-sm",
                                    style: "{style}",
                                }
                            }
                        }
                    }
                }
            }

            // Text below covers
            div { class: "mt-3 text-center w-full max-w-[160px]",
                Link {
                    to: Route::SeriesDetailPage { token: data.token.clone() },
                    class: "text-sm font-medium text-gray-900 hover:text-indigo-600 line-clamp-2 block",
                    "{data.name}"
                }
                if let Some(ref author_token) = data.first_author_token {
                    if let Some(ref author_name) = data.first_author_name {
                        Link {
                            to: Route::AuthorDetailPage { token: author_token.clone() },
                            class: "text-xs text-gray-500 hover:text-indigo-600 truncate block mt-0.5",
                            "{author_name}"
                        }
                    }
                }
                {
                    let count_label = if data.book_count == 1 {
                        "1 book".to_string()
                    } else {
                        format!("{} books", data.book_count)
                    };
                    rsx! {
                        p { class: "text-xs text-gray-400 mt-0.5", "{count_label}" }
                    }
                }
            }
        }
    }
}
