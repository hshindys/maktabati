-- Search projections are populated and maintained by the storage repositories. These
-- external-content triggers keep each FTS5 index in the same transaction as its projection.

ALTER TABLE search_document ADD COLUMN subtitle_norm TEXT NOT NULL DEFAULT '';
ALTER TABLE search_document ADD COLUMN original_title_norm TEXT NOT NULL DEFAULT '';
ALTER TABLE search_document ADD COLUMN series_norm TEXT NOT NULL DEFAULT '';
ALTER TABLE search_document ADD COLUMN isbn_norm TEXT NOT NULL DEFAULT '';

ALTER TABLE quotes ADD COLUMN note_norm TEXT NOT NULL DEFAULT '';

DROP TABLE fts_book_norm;
CREATE VIRTUAL TABLE fts_book_norm USING fts5(
  title_norm, subtitle_norm, original_title_norm, authors_norm, publisher_norm, series_norm,
  tags_norm, categories_norm, isbn_norm, description_norm, identifiers_norm,
  content='search_document', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2", columnsize=0
);

DROP TABLE fts_note;
DROP TABLE search_note_doc;
CREATE TABLE search_note_doc (
  rowid     INTEGER PRIMARY KEY AUTOINCREMENT,
  page_id   TEXT NOT NULL UNIQUE REFERENCES notes_page(id) ON DELETE CASCADE,
  book_id   TEXT,
  text      TEXT NOT NULL,
  text_norm TEXT NOT NULL
);
CREATE VIRTUAL TABLE fts_note USING fts5(
  text, text_norm, content='search_note_doc', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2", columnsize=0
);

DROP TABLE fts_quote;
CREATE VIRTUAL TABLE fts_quote USING fts5(
  text_norm, note_norm, content='quotes', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2", columnsize=0
);

CREATE VIRTUAL TABLE fts_author USING fts5(
  name, name_norm, name_original, name_original_norm, bio,
  content='authors', content_rowid='rowid',
  tokenize="unicode61 remove_diacritics 2", columnsize=0
);

CREATE TABLE search_index_state (
  id      INTEGER PRIMARY KEY CHECK (id = 1),
  version INTEGER NOT NULL
);

CREATE TRIGGER search_document_ai AFTER INSERT ON search_document BEGIN
  INSERT INTO fts_book(rowid, title, authors, publisher, series, tags, categories, isbn,
                       description, original_title, subtitle, identifiers)
  VALUES (new.rowid, new.title, new.authors, new.publisher, new.series, new.tags,
          new.categories, new.isbn, new.description, new.original_title, new.subtitle,
          new.identifiers);
  INSERT INTO fts_book_norm(rowid, title_norm, subtitle_norm, original_title_norm, authors_norm,
                            publisher_norm, series_norm, tags_norm, categories_norm, isbn_norm,
                            description_norm, identifiers_norm)
  VALUES (new.rowid, new.title_norm, new.subtitle_norm, new.original_title_norm, new.authors_norm,
          new.publisher_norm, new.series_norm, new.tags_norm, new.categories_norm, new.isbn_norm,
          new.description_norm, new.identifiers_norm);
END;

CREATE TRIGGER search_document_ad AFTER DELETE ON search_document BEGIN
  INSERT INTO fts_book(fts_book, rowid, title, authors, publisher, series, tags, categories, isbn,
                       description, original_title, subtitle, identifiers)
  VALUES ('delete', old.rowid, old.title, old.authors, old.publisher, old.series, old.tags,
          old.categories, old.isbn, old.description, old.original_title, old.subtitle,
          old.identifiers);
  INSERT INTO fts_book_norm(fts_book_norm, rowid, title_norm, subtitle_norm, original_title_norm,
                            authors_norm, publisher_norm, series_norm, tags_norm, categories_norm,
                            isbn_norm, description_norm, identifiers_norm)
  VALUES ('delete', old.rowid, old.title_norm, old.subtitle_norm, old.original_title_norm,
          old.authors_norm, old.publisher_norm, old.series_norm, old.tags_norm, old.categories_norm,
          old.isbn_norm, old.description_norm, old.identifiers_norm);
END;

CREATE TRIGGER search_document_au AFTER UPDATE ON search_document BEGIN
  INSERT INTO fts_book(fts_book, rowid, title, authors, publisher, series, tags, categories, isbn,
                       description, original_title, subtitle, identifiers)
  VALUES ('delete', old.rowid, old.title, old.authors, old.publisher, old.series, old.tags,
          old.categories, old.isbn, old.description, old.original_title, old.subtitle,
          old.identifiers);
  INSERT INTO fts_book(rowid, title, authors, publisher, series, tags, categories, isbn,
                       description, original_title, subtitle, identifiers)
  VALUES (new.rowid, new.title, new.authors, new.publisher, new.series, new.tags, new.categories,
          new.isbn, new.description, new.original_title, new.subtitle, new.identifiers);
  INSERT INTO fts_book_norm(fts_book_norm, rowid, title_norm, subtitle_norm, original_title_norm,
                            authors_norm, publisher_norm, series_norm, tags_norm, categories_norm,
                            isbn_norm, description_norm, identifiers_norm)
  VALUES ('delete', old.rowid, old.title_norm, old.subtitle_norm, old.original_title_norm,
          old.authors_norm, old.publisher_norm, old.series_norm, old.tags_norm, old.categories_norm,
          old.isbn_norm, old.description_norm, old.identifiers_norm);
  INSERT INTO fts_book_norm(rowid, title_norm, subtitle_norm, original_title_norm, authors_norm,
                            publisher_norm, series_norm, tags_norm, categories_norm, isbn_norm,
                            description_norm, identifiers_norm)
  VALUES (new.rowid, new.title_norm, new.subtitle_norm, new.original_title_norm, new.authors_norm,
          new.publisher_norm, new.series_norm, new.tags_norm, new.categories_norm, new.isbn_norm,
          new.description_norm, new.identifiers_norm);
END;

CREATE TRIGGER books_search_status_au
AFTER UPDATE OF reading_status, updated_at ON books BEGIN
  UPDATE search_document SET status = new.reading_status, updated_at = new.updated_at
  WHERE book_id = new.id;
END;

CREATE TRIGGER search_note_doc_ai AFTER INSERT ON search_note_doc BEGIN
  INSERT INTO fts_note(rowid, text, text_norm) VALUES (new.rowid, new.text, new.text_norm);
END;

CREATE TRIGGER search_note_doc_ad AFTER DELETE ON search_note_doc BEGIN
  INSERT INTO fts_note(fts_note, rowid, text, text_norm)
  VALUES ('delete', old.rowid, old.text, old.text_norm);
END;

CREATE TRIGGER search_note_doc_au AFTER UPDATE ON search_note_doc BEGIN
  INSERT INTO fts_note(fts_note, rowid, text, text_norm)
  VALUES ('delete', old.rowid, old.text, old.text_norm);
  INSERT INTO fts_note(rowid, text, text_norm) VALUES (new.rowid, new.text, new.text_norm);
END;

CREATE TRIGGER quotes_fts_ai AFTER INSERT ON quotes BEGIN
  INSERT INTO fts_quote(rowid, text_norm, note_norm)
  VALUES (new.rowid, new.text_norm, new.note_norm);
END;

CREATE TRIGGER quotes_fts_ad AFTER DELETE ON quotes BEGIN
  INSERT INTO fts_quote(fts_quote, rowid, text_norm, note_norm)
  VALUES ('delete', old.rowid, old.text_norm, old.note_norm);
END;

CREATE TRIGGER quotes_fts_au AFTER UPDATE ON quotes BEGIN
  INSERT INTO fts_quote(fts_quote, rowid, text_norm, note_norm)
  VALUES ('delete', old.rowid, old.text_norm, old.note_norm);
  INSERT INTO fts_quote(rowid, text_norm, note_norm)
  VALUES (new.rowid, new.text_norm, new.note_norm);
END;

CREATE TRIGGER authors_fts_ai AFTER INSERT ON authors BEGIN
  INSERT INTO fts_author(rowid, name, name_norm, name_original, name_original_norm, bio)
  VALUES (new.rowid, new.name, new.name_norm, new.name_original, new.name_original_norm, new.bio);
END;

CREATE TRIGGER authors_fts_ad AFTER DELETE ON authors BEGIN
  INSERT INTO fts_author(fts_author, rowid, name, name_norm, name_original, name_original_norm,
                         bio)
  VALUES ('delete', old.rowid, old.name, old.name_norm, old.name_original, old.name_original_norm,
          old.bio);
END;

CREATE TRIGGER authors_fts_au AFTER UPDATE ON authors BEGIN
  INSERT INTO fts_author(fts_author, rowid, name, name_norm, name_original, name_original_norm,
                         bio)
  VALUES ('delete', old.rowid, old.name, old.name_norm, old.name_original, old.name_original_norm,
          old.bio);
  INSERT INTO fts_author(rowid, name, name_norm, name_original, name_original_norm, bio)
  VALUES (new.rowid, new.name, new.name_norm, new.name_original, new.name_original_norm, new.bio);
END;

CREATE TRIGGER book_authors_search_ai AFTER INSERT ON book_authors BEGIN
  UPDATE search_document SET
    authors = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT a.name AS name FROM book_authors ba JOIN authors a ON a.id = ba.author_id
      WHERE ba.book_id = new.book_id ORDER BY ba.position
    )), ''),
    authors_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT a.name_norm AS name_norm FROM book_authors ba JOIN authors a ON a.id = ba.author_id
      WHERE ba.book_id = new.book_id ORDER BY ba.position
    )), '')
  WHERE book_id = new.book_id;
END;

CREATE TRIGGER book_authors_search_ad AFTER DELETE ON book_authors BEGIN
  UPDATE search_document SET
    authors = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT a.name AS name FROM book_authors ba JOIN authors a ON a.id = ba.author_id
      WHERE ba.book_id = old.book_id ORDER BY ba.position
    )), ''),
    authors_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT a.name_norm AS name_norm FROM book_authors ba JOIN authors a ON a.id = ba.author_id
      WHERE ba.book_id = old.book_id ORDER BY ba.position
    )), '')
  WHERE book_id = old.book_id;
END;

CREATE TRIGGER authors_search_au AFTER UPDATE ON authors BEGIN
  UPDATE search_document SET
    authors = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT a.name AS name FROM book_authors ba JOIN authors a ON a.id = ba.author_id
      WHERE ba.book_id = search_document.book_id ORDER BY ba.position
    )), ''),
    authors_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT a.name_norm AS name_norm FROM book_authors ba JOIN authors a ON a.id = ba.author_id
      WHERE ba.book_id = search_document.book_id ORDER BY ba.position
    )), '')
  WHERE book_id IN (SELECT book_id FROM book_authors WHERE author_id = new.id);
END;

CREATE TRIGGER book_tags_search_ai AFTER INSERT ON book_tags BEGIN
  UPDATE search_document SET
    tags = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT t.name AS name FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.book_id = new.book_id ORDER BY t.name_norm
    )), ''),
    tags_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT t.name_norm AS name_norm FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.book_id = new.book_id ORDER BY t.name_norm
    )), '')
  WHERE book_id = new.book_id;
END;

CREATE TRIGGER book_tags_search_ad AFTER DELETE ON book_tags BEGIN
  UPDATE search_document SET
    tags = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT t.name AS name FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.book_id = old.book_id ORDER BY t.name_norm
    )), ''),
    tags_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT t.name_norm AS name_norm FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.book_id = old.book_id ORDER BY t.name_norm
    )), '')
  WHERE book_id = old.book_id;
END;

CREATE TRIGGER tags_search_au AFTER UPDATE ON tags BEGIN
  UPDATE search_document SET
    tags = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT t.name AS name FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.book_id = search_document.book_id ORDER BY t.name_norm
    )), ''),
    tags_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT t.name_norm AS name_norm FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.book_id = search_document.book_id ORDER BY t.name_norm
    )), '')
  WHERE book_id IN (SELECT book_id FROM book_tags WHERE tag_id = new.id);
END;

CREATE TRIGGER book_categories_search_ai AFTER INSERT ON book_categories BEGIN
  UPDATE search_document SET
    categories = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT c.name_ar || ' ' || c.name_en AS name
      FROM book_categories bc JOIN categories c ON c.id = bc.category_id
      WHERE bc.book_id = new.book_id ORDER BY bc.position
    )), ''),
    categories_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT c.name_norm AS name_norm
      FROM book_categories bc JOIN categories c ON c.id = bc.category_id
      WHERE bc.book_id = new.book_id ORDER BY bc.position
    )), '')
  WHERE book_id = new.book_id;
END;

CREATE TRIGGER book_categories_search_ad AFTER DELETE ON book_categories BEGIN
  UPDATE search_document SET
    categories = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT c.name_ar || ' ' || c.name_en AS name
      FROM book_categories bc JOIN categories c ON c.id = bc.category_id
      WHERE bc.book_id = old.book_id ORDER BY bc.position
    )), ''),
    categories_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT c.name_norm AS name_norm
      FROM book_categories bc JOIN categories c ON c.id = bc.category_id
      WHERE bc.book_id = old.book_id ORDER BY bc.position
    )), '')
  WHERE book_id = old.book_id;
END;

CREATE TRIGGER categories_search_au AFTER UPDATE ON categories BEGIN
  UPDATE search_document SET
    categories = COALESCE((SELECT group_concat(name, ' ') FROM (
      SELECT c.name_ar || ' ' || c.name_en AS name
      FROM book_categories bc JOIN categories c ON c.id = bc.category_id
      WHERE bc.book_id = search_document.book_id ORDER BY bc.position
    )), ''),
    categories_norm = COALESCE((SELECT group_concat(name_norm, ' ') FROM (
      SELECT c.name_norm AS name_norm
      FROM book_categories bc JOIN categories c ON c.id = bc.category_id
      WHERE bc.book_id = search_document.book_id ORDER BY bc.position
    )), '')
  WHERE book_id IN (SELECT book_id FROM book_categories WHERE category_id = new.id);
END;

CREATE TRIGGER publishers_search_au AFTER UPDATE ON publishers BEGIN
  UPDATE search_document
  SET publisher = new.name, publisher_norm = new.name_norm
  WHERE book_id IN (SELECT id FROM books WHERE publisher_id = new.id);
END;

CREATE TRIGGER publishers_search_ad AFTER DELETE ON publishers BEGIN
  UPDATE search_document SET publisher = NULL, publisher_norm = ''
  WHERE publisher_norm = old.name_norm;
END;

CREATE TRIGGER series_search_au AFTER UPDATE ON series BEGIN
  UPDATE search_document SET series = new.name, series_norm = new.name_norm
  WHERE book_id IN (SELECT id FROM books WHERE series_id = new.id);
END;

CREATE TRIGGER series_search_ad AFTER DELETE ON series BEGIN
  UPDATE search_document SET series = NULL, series_norm = ''
  WHERE series_norm = old.name_norm;
END;
