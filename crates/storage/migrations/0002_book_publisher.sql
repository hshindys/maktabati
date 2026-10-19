-- 0002: a book knows its publisher.
--
-- `0001_init` created `publishers` and `BookInput` already carries `publisher_id`, but the
-- two were never linked — a book had nowhere to record its publishing house even though the
-- per-book page shows one. One publisher per book in v1, mirroring `series_id` and for the
-- same reason (`docs/03-data-model.md` §3).
--
-- SQLite accepts a `REFERENCES` clause on `ADD COLUMN` only when the column defaults to
-- NULL, which this one does.

ALTER TABLE books ADD COLUMN publisher_id TEXT REFERENCES publishers(id) ON DELETE SET NULL;

CREATE INDEX ix_books_publisher ON books(publisher_id);
