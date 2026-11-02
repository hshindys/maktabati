-- 0003: cover provenance grows two internet providers.
--
-- The bulk cover fetch now consults four keyless sources (Open Library covers, Google
-- Books volumes, Amazon's ISBN-keyed image host, Internet Archive) and records which one
-- supplied each cover in `books.cover_source`. The column's `CHECK` only knew the first
-- two plus the local values, so it is widened to include `amazon` and `internet_archive`.
--
-- SQLite cannot alter a CHECK in place; the standard recipe (SQLite docs, "Making Other
-- Kinds Of Table Schema Changes") rebuilds the table:
--   create `books_new` → copy every column row-for-row → drop `books` → rename.
--
-- Two things about this file make it different from an ordinary migration:
-- - SQLite runs an implicit DELETE for `DROP TABLE` and enforces the referencing tables'
--   foreign keys while it does. `books` is the parent of fifteen child tables, so the
--   rebuild needs `PRAGMA foreign_keys = OFF` — a pragma that cannot be toggled inside a
--   transaction. The migration runner therefore applies *this file* outside the standard
--   transaction, with foreign keys off, but the SQL below still runs inside ONE
--   transaction (see `migrate.rs`), so a crash mid-rebuild rolls back cleanly.
-- - The DDL below replicates the `books` schema of 0001+0002 verbatim — every column,
--   every CHECK, the publisher FK from 0002 — and changes only the `cover_source` CHECK.
--   The copies are explicit, column for column, in matching order.
-- After the rename the runner re-checks `PRAGMA foreign_key_check` before restoring
-- foreign keys, so an unhappy rebuild is an error, not silent corruption.

CREATE TABLE books_new (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  subtitle       TEXT,
  original_title TEXT,
  title_norm     TEXT NOT NULL,
  language_code  TEXT NOT NULL REFERENCES languages(code),
  isbn10         TEXT,
  isbn13         TEXT,
  edition        TEXT,
  publication_year INTEGER,
  page_count     INTEGER CHECK (page_count IS NULL OR page_count > 0),
  format         TEXT NOT NULL DEFAULT 'print'
                   CHECK (format IN ('print','ebook','audiobook','pdf','other')),
  description    TEXT,
  series_id      TEXT REFERENCES series(id) ON DELETE SET NULL,
  series_order   REAL,
  -- physical location (collector persona)
  room           TEXT,
  shelf          TEXT,
  box            TEXT,
  -- acquisition
  acquired_date  TEXT,
  acquired_from  TEXT,
  price_minor    INTEGER,
  price_currency TEXT,
  condition      TEXT CHECK (condition IS NULL OR condition IN
                   ('new','like_new','very_good','good','fair','poor')),
  -- cover
  cover_path     TEXT,
  cover_thumb    TEXT,
  cover_source   TEXT CHECK (cover_source IS NULL OR cover_source IN
                   ('manual','openlibrary','googlebooks','amazon','internet_archive',
                    'embedded','generated')),
  -- current reading state (denormalized for indexed filtering; docs/03-data-model.md §4.3)
  reading_status TEXT NOT NULL DEFAULT 'to_read'
                   CHECK (reading_status IN ('to_read','reading','finished','paused','abandoned')),
  rating         INTEGER CHECK (rating IS NULL OR (rating BETWEEN 0 AND 10)),
  current_page   INTEGER,
  progress_pct   INTEGER NOT NULL DEFAULT 0 CHECK (progress_pct BETWEEN 0 AND 100),
  source         TEXT CHECK (source IS NULL OR source IN
                   ('manual','isbn','folder_scan','csv','goodreads','calibre','json')),
  external_ids   TEXT CHECK (external_ids IS NULL OR json_valid(external_ids)),
  -- publisher link (added by 0002; declared in the same shape so the copy is 1:1)
  publisher_id   TEXT REFERENCES publishers(id) ON DELETE SET NULL,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);

INSERT INTO books_new (
  id, title, subtitle, original_title, title_norm, language_code,
  isbn10, isbn13, edition, publication_year, page_count, format, description,
  series_id, series_order, room, shelf, box,
  acquired_date, acquired_from, price_minor, price_currency, condition,
  cover_path, cover_thumb, cover_source,
  reading_status, rating, current_page, progress_pct, source, external_ids,
  publisher_id, created_at, updated_at
)
SELECT
  id, title, subtitle, original_title, title_norm, language_code,
  isbn10, isbn13, edition, publication_year, page_count, format, description,
  series_id, series_order, room, shelf, box,
  acquired_date, acquired_from, price_minor, price_currency, condition,
  cover_path, cover_thumb, cover_source,
  reading_status, rating, current_page, progress_pct, source, external_ids,
  publisher_id, created_at, updated_at
FROM books;

DROP TABLE books;
ALTER TABLE books_new RENAME TO books;

-- Every index 0001 defined on `books` (they ride the dropped table away, so they are
-- recreated here; the definitions are copied verbatim).
CREATE UNIQUE INDEX ux_books_isbn13 ON books(isbn13) WHERE isbn13 IS NOT NULL;
CREATE UNIQUE INDEX ux_books_isbn10 ON books(isbn10) WHERE isbn10 IS NOT NULL;
CREATE INDEX ix_books_title_norm ON books(title_norm);
CREATE INDEX ix_books_status     ON books(reading_status);
CREATE INDEX ix_books_year       ON books(publication_year);
CREATE INDEX ix_books_rating     ON books(rating);
CREATE INDEX ix_books_progress   ON books(progress_pct);
CREATE INDEX ix_books_updated    ON books(updated_at DESC);
CREATE INDEX ix_books_created    ON books(created_at DESC);
CREATE INDEX ix_books_series     ON books(series_id, series_order);
CREATE INDEX ix_books_loc        ON books(room, shelf, box);
