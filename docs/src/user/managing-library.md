# Managing Your Library

## Adding Books

BookBoss acquires books through a watched directory. The pipeline runs automatically in the background.

### Workflow

1. **Drop a file** into the directory configured as `BOOKBOSS__IMPORT__WATCH_DIRECTORY`
2. **BookBoss picks it up** — the scanner runs on a configurable interval (default: 60 seconds) and hashes each new file to avoid duplicates
3. **Metadata is extracted** — for EPUB files, embedded metadata is read from the OPF inside the archive; other formats fall through to the provider lookup
4. **Provider enrichment** — if the file contains an ISBN, BookBoss queries Open Library for additional metadata and a cover image
5. **Review queue** — the book lands in the **Incoming** section of the library (requires the _Approve Imports_ capability)

### Reviewing and Approving

Navigate to **Library → Incoming** to see books awaiting review.

Each review page shows three columns: the field name, the current extracted value (editable), and the value fetched from the metadata provider. Use the **←** copy buttons to pull individual fields from the provider into the current value.

- **Fetch provider data** — re-queries the provider using the current identifiers in the form
- **Approve** — commits the edited metadata, moves the book to your library, and sets its status to _Available_
- **Reject** — discards the import
- **Cancel** — returns to the Incoming list without changes

### File Storage

Approved books are stored under `BOOKBOSS__LIBRARY__LIBRARY_PATH` with the layout:

```
{library_path}/
└── BK_<token>/
    ├── <slug>.<ext>      # the book file (e.g. the-hobbit.epub)
    ├── cover.jpg         # cover image
    └── metadata.opf      # OPF sidecar with all metadata
```

### Duplicate Detection

Files are SHA-256 hashed before ingestion. If a file with the same hash already exists in the library, the import is skipped automatically.
