use dioxus::prelude::*;

use crate::routes::books_page::TreeCategory;

#[component]
pub(crate) fn TreeExplorer(categories: Vec<TreeCategory>) -> Element {
    rsx! {
        aside { class: "w-56 shrink-0 bg-white border-r border-gray-200 overflow-y-auto p-4",
            h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-3",
                "Explorer"
            }
            for category in &categories {
                TreeCategoryNode { category: category.clone() }
            }
        }
    }
}

#[component]
fn TreeCategoryNode(category: TreeCategory) -> Element {
    let mut expanded = use_signal(|| true);

    rsx! {
        div { class: "mb-2",
            button {
                class: "flex items-center gap-1 text-sm font-medium text-gray-700 hover:text-indigo-600 w-full text-left",
                onclick: move |_| expanded.toggle(),
                span { class: "text-xs",
                    if expanded() { "\u{25BE}" } else { "\u{25B8}" }
                }
                "{category.name}"
            }
            if expanded() {
                ul { class: "ml-4 mt-1 space-y-0.5",
                    for item in &category.items {
                        li { class: "text-sm text-gray-600 hover:text-indigo-600 cursor-pointer py-0.5 px-1 rounded hover:bg-indigo-50",
                            "{item}"
                        }
                    }
                }
            }
        }
    }
}
