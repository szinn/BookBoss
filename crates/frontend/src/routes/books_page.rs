use dioxus::prelude::*;

use crate::components::{BookTable, DetailPanel, TreeExplorer};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Book {
    pub title: String,
    pub author: String,
    pub year: u16,
    pub genre: String,
    pub pages: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TreeCategory {
    pub name: String,
    pub items: Vec<String>,
}

fn sample_books() -> Vec<Book> {
    vec![
        Book {
            title: "The Pragmatic Programmer".into(),
            author: "Hunt & Thomas".into(),
            year: 1999,
            genre: "Software".into(),
            pages: 352,
        },
        Book {
            title: "Domain-Driven Design".into(),
            author: "Eric Evans".into(),
            year: 2003,
            genre: "Software".into(),
            pages: 560,
        },
        Book {
            title: "Clean Code".into(),
            author: "Robert C. Martin".into(),
            year: 2008,
            genre: "Software".into(),
            pages: 464,
        },
        Book {
            title: "Designing Data-Intensive Applications".into(),
            author: "Martin Kleppmann".into(),
            year: 2017,
            genre: "Systems".into(),
            pages: 616,
        },
        Book {
            title: "Rust in Action".into(),
            author: "Tim McNamara".into(),
            year: 2021,
            genre: "Programming".into(),
            pages: 456,
        },
    ]
}

fn sample_categories() -> Vec<TreeCategory> {
    vec![
        TreeCategory {
            name: "Genres".into(),
            items: vec!["Software".into(), "Systems".into(), "Programming".into()],
        },
        TreeCategory {
            name: "Decades".into(),
            items: vec!["1990s".into(), "2000s".into(), "2010s+".into()],
        },
        TreeCategory {
            name: "Collections".into(),
            items: vec!["Favorites".into(), "To Read".into(), "Archived".into()],
        },
    ]
}

#[component]
pub(crate) fn BooksPage() -> Element {
    let selected_book: Signal<Option<Book>> = use_signal(|| None);
    use_context_provider(|| selected_book);

    let books = sample_books();
    let categories = sample_categories();

    rsx! {
        TreeExplorer { categories }
        BookTable { books }
        DetailPanel {}
    }
}
