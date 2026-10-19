-- 0001_init.sql — canonical schema for Maktabati / مكتبتي
-- Mirrors docs/03-data-model.md §3. A test in `storage` asserts every Rust enum variant in
-- `domain::enums` appears in this file, so the CHECK constraints cannot drift from the code.
--
-- Conventions:
--   * PKs are TEXT UUID v7 (time-sortable).
--   * *_at are INTEGER epoch milliseconds UTC; *_date are TEXT 'YYYY-MM-DD'.
--   * Ratings are INTEGER 0..=10 (half-star units); money is integer minor units.
--   * *_norm columns hold the Arabic-aware normalized mirror (domain::text::normalize).

-- ─────────────────────────────────────────────────────────────────────────────
-- Reference data
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE languages (
  code        TEXT PRIMARY KEY,           -- 'ar', 'en', 'fr'…
  name_ar     TEXT NOT NULL,
  name_en     TEXT NOT NULL,
  direction   TEXT NOT NULL CHECK (direction IN ('rtl','ltr')),
  is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  position    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE publishers (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  name_norm   TEXT NOT NULL,
  country     TEXT,
  city        TEXT,
  website     TEXT,
  notes       TEXT,
  cover_path  TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_publishers_name_norm ON publishers(name_norm);

CREATE TABLE authors (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  name_norm     TEXT NOT NULL,
  name_original TEXT,
  name_original_norm TEXT,
  bio           TEXT,
  nationality   TEXT,
  birth_year    INTEGER,
  death_year    INTEGER,
  photo_path    TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_authors_name_norm ON authors(name_norm);
CREATE INDEX ix_authors_birth ON authors(birth_year);

CREATE TABLE series (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  name_norm   TEXT NOT NULL,
  description TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_series_name_norm ON series(name_norm);

CREATE TABLE categories (
  id          TEXT PRIMARY KEY,
  parent_id   TEXT REFERENCES categories(id) ON DELETE CASCADE,
  key         TEXT NOT NULL,              -- stable machine key, never localized
  name_ar     TEXT NOT NULL,
  name_en     TEXT NOT NULL,
  name_norm   TEXT NOT NULL,
  path        TEXT NOT NULL,              -- materialized path '/literature/novels'
  depth       INTEGER NOT NULL DEFAULT 0,
  position    INTEGER NOT NULL DEFAULT 0,
  icon        TEXT,
  color       TEXT,
  is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0,1)),
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_categories_parent_key ON categories(parent_id, key);
CREATE UNIQUE INDEX ux_categories_path ON categories(path);
CREATE INDEX ix_categories_parent ON categories(parent_id, position);

CREATE TABLE tags (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  name_norm   TEXT NOT NULL,
  color       TEXT,
  created_at  INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_tags_name_norm ON tags(name_norm);

-- ─────────────────────────────────────────────────────────────────────────────
-- Books
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE books (
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
                   ('manual','openlibrary','googlebooks','embedded','generated')),
  -- current reading state (denormalized for indexed filtering; docs/03-data-model.md §4.3)
  reading_status TEXT NOT NULL DEFAULT 'to_read'
                   CHECK (reading_status IN ('to_read','reading','finished','paused','abandoned')),
  rating         INTEGER CHECK (rating IS NULL OR (rating BETWEEN 0 AND 10)),
  current_page   INTEGER,
  progress_pct   INTEGER NOT NULL DEFAULT 0 CHECK (progress_pct BETWEEN 0 AND 100),
  source         TEXT CHECK (source IS NULL OR source IN
                   ('manual','isbn','folder_scan','csv','goodreads','calibre','json')),
  external_ids   TEXT CHECK (external_ids IS NULL OR json_valid(external_ids)),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
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

CREATE TABLE book_authors (
  book_id    TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  author_id  TEXT NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
  role       TEXT NOT NULL DEFAULT 'author'
               CHECK (role IN ('author','translator','editor','illustrator','foreword','other')),
  position   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (book_id, author_id, role)
) WITHOUT ROWID;
CREATE INDEX ix_book_authors_author ON book_authors(author_id, role);

CREATE TABLE book_categories (
  book_id     TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (book_id, category_id)
) WITHOUT ROWID;
CREATE INDEX ix_book_categories_cat ON book_categories(category_id);

CREATE TABLE book_tags (
  book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  tag_id  TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (book_id, tag_id)
) WITHOUT ROWID;
CREATE INDEX ix_book_tags_tag ON book_tags(tag_id);

-- Translators have NO separate table: they are `book_authors` rows with role='translator'.

-- ─────────────────────────────────────────────────────────────────────────────
-- Reading
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE reading_cycles (
  id           TEXT PRIMARY KEY,
  book_id      TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  started_at   INTEGER,
  finished_at  INTEGER,
  start_page   INTEGER,
  end_page     INTEGER,
  source       TEXT NOT NULL DEFAULT 'manual'
                 CHECK (source IN ('manual','reader','import')),
  note         TEXT,
  created_at   INTEGER NOT NULL
);
CREATE INDEX ix_cycles_book ON reading_cycles(book_id, started_at DESC);

CREATE TABLE reading_sessions (
  id            TEXT PRIMARY KEY,
  book_id       TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  cycle_id      TEXT REFERENCES reading_cycles(id) ON DELETE SET NULL,
  session_date  TEXT NOT NULL,            -- YYYY-MM-DD, the user's calendar day
  started_at    INTEGER,
  ended_at      INTEGER,
  pages_read    INTEGER CHECK (pages_read IS NULL OR pages_read >= 0),
  minutes       INTEGER CHECK (minutes   IS NULL OR minutes   >= 0),
  note          TEXT,
  created_at    INTEGER NOT NULL
);
CREATE INDEX ix_sessions_date ON reading_sessions(session_date);
CREATE INDEX ix_sessions_book ON reading_sessions(book_id, session_date);
CREATE INDEX ix_sessions_time ON reading_sessions(created_at DESC);

CREATE TABLE loans (
  id           TEXT PRIMARY KEY,
  book_id      TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  borrower     TEXT NOT NULL,
  phone        TEXT,
  borrowed_at  TEXT NOT NULL,
  due_date     TEXT,
  returned_at  TEXT,
  reminder_at  TEXT,
  notes        TEXT,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX ix_loans_open ON loans(book_id) WHERE returned_at IS NULL;
CREATE INDEX ix_loans_due  ON loans(due_date) WHERE returned_at IS NULL;

CREATE TABLE reading_goals (
  id            TEXT PRIMARY KEY,
  scope         TEXT NOT NULL CHECK (scope IN ('year','month')),
  year          INTEGER NOT NULL,
  month         INTEGER CHECK (month IS NULL OR (month BETWEEN 1 AND 12)),
  target_books  INTEGER,
  target_pages  INTEGER,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_goals ON reading_goals(scope, year, month);

CREATE TABLE activity_log (
  id          TEXT PRIMARY KEY,
  occurred_at INTEGER NOT NULL,
  kind        TEXT NOT NULL,
  book_id     TEXT REFERENCES books(id) ON DELETE CASCADE,
  payload     TEXT CHECK (payload IS NULL OR json_valid(payload))
);
CREATE INDEX ix_activity_time ON activity_log(occurred_at DESC);
CREATE INDEX ix_activity_book ON activity_log(book_id, occurred_at DESC);

-- ─────────────────────────────────────────────────────────────────────────────
-- Evaluation: reviews, comments, rubric
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE reviews (
  id          TEXT PRIMARY KEY,
  book_id     TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  rating      INTEGER CHECK (rating IS NULL OR (rating BETWEEN 0 AND 10)),
  body        TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX ix_reviews_book ON reviews(book_id, created_at DESC);

CREATE TABLE review_comments (
  id           TEXT PRIMARY KEY,
  review_id    TEXT NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
  body         TEXT NOT NULL,
  commented_at INTEGER NOT NULL,
  created_at   INTEGER NOT NULL
);
CREATE INDEX ix_review_comments ON review_comments(review_id, commented_at);

CREATE TABLE rating_criteria (
  id             TEXT PRIMARY KEY,
  key            TEXT NOT NULL,
  name_ar        TEXT NOT NULL,
  name_en        TEXT NOT NULL,
  description_ar TEXT,
  description_en TEXT,
  weight         REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0),
  position       INTEGER NOT NULL DEFAULT 0,
  is_active      INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  is_system      INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0,1)),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_criteria_key ON rating_criteria(key);

CREATE TABLE rubric_scores (
  book_id      TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  criterion_id TEXT NOT NULL REFERENCES rating_criteria(id) ON DELETE CASCADE,
  score        INTEGER NOT NULL CHECK (score BETWEEN 0 AND 10),
  note         TEXT,
  updated_at   INTEGER NOT NULL,
  PRIMARY KEY (book_id, criterion_id)
) WITHOUT ROWID;
CREATE INDEX ix_rubric_criterion ON rubric_scores(criterion_id, score);

-- ─────────────────────────────────────────────────────────────────────────────
-- Notes (ADR-0006)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE notes_page (
  id             TEXT PRIMARY KEY,
  book_id        TEXT REFERENCES books(id) ON DELETE CASCADE,
  parent_page_id TEXT REFERENCES notes_page(id) ON DELETE CASCADE,
  title          TEXT NOT NULL DEFAULT '',
  title_norm     TEXT NOT NULL DEFAULT '',
  icon           TEXT,
  is_template    INTEGER NOT NULL DEFAULT 0 CHECK (is_template IN (0,1)),
  template_key   TEXT,
  is_pinned      INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0,1)),
  position       INTEGER NOT NULL DEFAULT 0,
  rev            INTEGER NOT NULL DEFAULT 0,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
);
CREATE INDEX ix_pages_book   ON notes_page(book_id, position);
CREATE INDEX ix_pages_parent ON notes_page(parent_page_id, position);
CREATE INDEX ix_pages_title  ON notes_page(title_norm);

CREATE TABLE note_block (
  id              TEXT PRIMARY KEY,
  page_id         TEXT NOT NULL REFERENCES notes_page(id) ON DELETE CASCADE,
  parent_block_id TEXT REFERENCES note_block(id) ON DELETE CASCADE,
  position        TEXT NOT NULL,         -- fractional index
  type            TEXT NOT NULL CHECK (type IN (
      'paragraph','heading_1','heading_2','heading_3',
      'bulleted_list','numbered_list','todo','quote','code',
      'callout','toggle','table','divider','image','page_link','bookmark')),
  payload         TEXT NOT NULL CHECK (json_valid(payload)),
  rev             INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_blocks_pos ON note_block(page_id, position);
CREATE INDEX ix_blocks_parent ON note_block(parent_block_id, position);
CREATE INDEX ix_blocks_page   ON note_block(page_id, position);

CREATE TABLE note_link (
  id            TEXT PRIMARY KEY,
  from_block_id TEXT NOT NULL REFERENCES note_block(id) ON DELETE CASCADE,
  target_kind   TEXT NOT NULL CHECK (target_kind IN ('page','book','quote')),
  target_id     TEXT NOT NULL,
  raw_text      TEXT NOT NULL,
  created_at    INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_links ON note_link(from_block_id, target_kind, target_id);
CREATE INDEX ix_links_target ON note_link(target_kind, target_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- Quotes and files
-- ─────────────────────────────────────────────────────────────────────────────

-- `file_id` references `book_files`, which is created below. SQLite resolves foreign keys at
-- runtime (PRAGMA foreign_keys=ON), so the forward reference is valid.
CREATE TABLE quotes (
  id           TEXT PRIMARY KEY,
  book_id      TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  file_id      TEXT REFERENCES book_files(id) ON DELETE SET NULL,
  text         TEXT NOT NULL,
  text_norm    TEXT NOT NULL,
  note         TEXT,
  page         INTEGER,
  chapter      TEXT,
  color        TEXT NOT NULL DEFAULT 'yellow'
                 CHECK (color IN ('yellow','green','blue','pink','red','purple')),
  position_ref TEXT,
  source       TEXT NOT NULL DEFAULT 'manual'
                 CHECK (source IN ('manual','reader','import')),
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX ix_quotes_book ON quotes(book_id, created_at DESC);
CREATE INDEX ix_quotes_page ON quotes(book_id, page);
CREATE INDEX ix_quotes_norm ON quotes(text_norm);

CREATE TABLE book_files (
  id               TEXT PRIMARY KEY,
  book_id          TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  kind             TEXT NOT NULL CHECK (kind IN ('epub','pdf','mobi','azw3','txt','audio','other')),
  original_name    TEXT NOT NULL,
  storage_mode     TEXT NOT NULL CHECK (storage_mode IN ('copy','reference')),
  rel_path         TEXT,
  abs_path         TEXT,
  sha256           TEXT NOT NULL,
  size_bytes       INTEGER NOT NULL,
  mime             TEXT,
  page_count       INTEGER,
  reader_position  TEXT,
  added_at         INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
);
CREATE INDEX ix_files_book ON book_files(book_id);
-- The same bytes are stored once in copy mode, so two books can share one blob.
CREATE UNIQUE INDEX ux_files_sha_copy ON book_files(sha256) WHERE storage_mode = 'copy';

-- ─────────────────────────────────────────────────────────────────────────────
-- Shelves, custom columns, settings, cache
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE shelves (
  id          TEXT PRIMARY KEY,
  name_ar     TEXT NOT NULL,
  name_en     TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'static' CHECK (kind IN ('static','smart')),
  query_json  TEXT CHECK (query_json IS NULL OR json_valid(query_json)),
  icon        TEXT,
  color       TEXT,
  is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0,1)),
  position    INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX ix_shelves_pos ON shelves(position);

CREATE TABLE shelf_books (
  shelf_id TEXT NOT NULL REFERENCES shelves(id) ON DELETE CASCADE,
  book_id  TEXT NOT NULL REFERENCES books(id)  ON DELETE CASCADE,
  position INTEGER NOT NULL DEFAULT 0,
  added_at INTEGER NOT NULL,
  PRIMARY KEY (shelf_id, book_id)
) WITHOUT ROWID;
CREATE INDEX ix_shelf_books_book ON shelf_books(book_id);

CREATE TABLE custom_column_def (
  id           TEXT PRIMARY KEY,
  key          TEXT NOT NULL,
  name_ar      TEXT NOT NULL,
  name_en      TEXT NOT NULL,
  data_type    TEXT NOT NULL CHECK (data_type IN
                 ('text','integer','float','bool','date','enum','rating')),
  is_multi     INTEGER NOT NULL DEFAULT 0 CHECK (is_multi IN (0,1)),
  is_required  INTEGER NOT NULL DEFAULT 0 CHECK (is_required IN (0,1)),
  display_json TEXT CHECK (display_json IS NULL OR json_valid(display_json)),
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE UNIQUE INDEX ux_custom_key ON custom_column_def(key);

CREATE TABLE custom_column_value (
  book_id    TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  column_id  TEXT NOT NULL REFERENCES custom_column_def(id) ON DELETE CASCADE,
  value_text TEXT,
  value_num  REAL,
  value_bool INTEGER CHECK (value_bool IS NULL OR value_bool IN (0,1)),
  PRIMARY KEY (book_id, column_id)
) WITHOUT ROWID;
CREATE INDEX ix_ccv_col_num ON custom_column_value(column_id, value_num);
CREATE INDEX ix_ccv_col_txt ON custom_column_value(column_id, value_text);

CREATE TABLE app_settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL CHECK (json_valid(value)),
  updated_at INTEGER NOT NULL
);

CREATE TABLE metadata_cache (
  provider   TEXT NOT NULL,
  query_kind TEXT NOT NULL,
  query_key  TEXT NOT NULL,
  payload    TEXT NOT NULL CHECK (json_valid(payload)),
  etag       TEXT,
  fetched_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  PRIMARY KEY (provider, query_kind, query_key)
) WITHOUT ROWID;
CREATE INDEX ix_meta_expiry ON metadata_cache(expires_at);

-- ─────────────────────────────────────────────────────────────────────────────
-- Search projection + FTS5 (docs/03-data-model.md §4, ADR-0004)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE search_document (
  rowid            INTEGER PRIMARY KEY AUTOINCREMENT,
  book_id          TEXT NOT NULL UNIQUE REFERENCES books(id) ON DELETE CASCADE,
  title            TEXT NOT NULL,
  title_norm       TEXT NOT NULL,
  subtitle         TEXT,
  original_title   TEXT,
  authors          TEXT NOT NULL DEFAULT '',
  authors_norm     TEXT NOT NULL DEFAULT '',
  publisher        TEXT,
  series           TEXT,
  tags             TEXT,
  categories       TEXT,
  isbn             TEXT,
  description      TEXT,
  identifiers      TEXT,
  year             INTEGER,
  format           TEXT,
  status           TEXT,
  language         TEXT,
  tags_norm        TEXT NOT NULL DEFAULT '',
  publisher_norm   TEXT NOT NULL DEFAULT '',
  categories_norm  TEXT NOT NULL DEFAULT '',
  description_norm TEXT NOT NULL DEFAULT '',
  identifiers_norm TEXT NOT NULL DEFAULT '',
  updated_at       INTEGER NOT NULL
);

CREATE VIRTUAL TABLE fts_book USING fts5(
  title, authors, publisher, series, tags, categories,
  isbn, description, original_title, subtitle, identifiers,
  content='search_document', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2",
  columnsize=0
);

CREATE VIRTUAL TABLE fts_book_norm USING fts5(
  title_norm, authors_norm, tags_norm, description_norm,
  content='search_document', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2",
  columnsize=0
);

CREATE TABLE search_note_doc (
  rowid     INTEGER PRIMARY KEY AUTOINCREMENT,
  block_id  TEXT NOT NULL UNIQUE REFERENCES note_block(id) ON DELETE CASCADE,
  page_id   TEXT NOT NULL,
  book_id   TEXT,
  text      TEXT NOT NULL,
  text_norm TEXT NOT NULL
);

CREATE VIRTUAL TABLE fts_note USING fts5(
  text, content='search_note_doc', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2", columnsize=0
);

CREATE VIRTUAL TABLE fts_quote USING fts5(
  text_norm, note, content='quotes', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2", columnsize=0
);
