//! `notes_page` + `note_block` — the notes documents (الملاحظات) and their blocks.
//!
//! The tables live in `0001_init.sql`; this module gives them the same typed repository the
//! other aggregates have. Two simplifications are deliberate (mirroring
//! `maktabati_domain::note`): block positions are sequential integers stored as decimal
//! strings in the TEXT column the schema allows, and `note_link` is untouched until
//! something links — no command writes it in v1.
//!
//! Every block write also touches the owning page (`rev` + `updated_at`), so surfacing
//! "this page changed" takes one read of the page row, not a block-by-block scan.

use std::collections::BTreeMap;

use maktabati_domain::json::Json;
use maktabati_domain::note::{NoteBlock, NoteBlockInput, NotePage, NotePageInput};
use maktabati_domain::NoteBlockType;

use crate::conn::Db;
use crate::Result;

/// Every `notes_page` column, in the order [`page_from_row`] reads them.
const PAGE_SELECT: &str = "SELECT id, book_id, parent_page_id, title, title_norm, icon,
                                  is_template, template_key, is_pinned, position, rev,
                                  created_at, updated_at
                           FROM notes_page";

/// Every `note_block` column, in the order [`block_from_row`] reads them.
const BLOCK_SELECT: &str = "SELECT id, page_id, parent_block_id, position, type, payload,
                                   rev, created_at, updated_at
                            FROM note_block";

/// Map one `notes_page` row onto a [`NotePage`].
fn page_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NotePage> {
    Ok(NotePage {
        id: row.get(0)?,
        book_id: row.get(1)?,
        parent_page_id: row.get(2)?,
        title: row.get(3)?,
        title_norm: row.get(4)?,
        icon: row.get(5)?,
        is_template: row.get::<_, i64>(6)? != 0,
        template_key: row.get(7)?,
        is_pinned: row.get::<_, i64>(8)? != 0,
        position: row.get(9)?,
        rev: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

/// Map one `note_block` row onto a [`NoteBlock`].
fn block_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NoteBlock> {
    let block_type: String = row.get(4)?;
    let payload: String = row.get(5)?;
    Ok(NoteBlock {
        id: row.get(0)?,
        page_id: row.get(1)?,
        parent_block_id: row.get(2)?,
        position: row.get(3)?,
        block_type: NoteBlockType::from_db(&block_type).unwrap_or(NoteBlockType::Paragraph),
        payload: serde_json::from_str::<serde_json::Value>(&payload)
            .map(Json::from)
            .unwrap_or(Json::Object(BTreeMap::new())),
        rev: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

/// Read a page row that must exist.
fn read_page(conn: &rusqlite::Connection, id: &str) -> Result<NotePage> {
    let sql = format!("{PAGE_SELECT} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([id])?;
    match rows.next()? {
        None => Err(crate::error::not_found("note_page", id)),
        Some(row) => Ok(page_from_row(row)?),
    }
}

/// Read a block row that must exist.
fn read_block(conn: &rusqlite::Connection, id: &str) -> Result<NoteBlock> {
    let sql = format!("{BLOCK_SELECT} WHERE id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([id])?;
    match rows.next()? {
        None => Err(crate::error::not_found("note_block", id)),
        Some(row) => Ok(block_from_row(row)?),
    }
}

/// Touch a page's `rev`/`updated_at` after a content write beneath it.
fn touch_page(conn: &rusqlite::Connection, page_id: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE notes_page SET rev = rev + 1, updated_at = ?2 WHERE id = ?1",
        rusqlite::params![page_id, now],
    )?;
    Ok(())
}

/// Store a note page according to [`NotePageInput`].
///
/// The page is appended after its siblings in the same scope (`book_id` + `parent_page_id`
/// pair), which is the whole ordering story pages need in v1. An unknown book or parent
/// page is refused before anything writes, so a bad id is a clean `storage.not_found`
/// rather than a raw foreign-key failure.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when `book_id` or `parent_page_id` names a missing row.
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn create_page(db: &Db, input: NotePageInput) -> Result<NotePage> {
    db.transact(|conn| {
        if let Some(book_id) = input.book_id.as_deref() {
            let known: Option<String> = conn
                .query_row("SELECT id FROM books WHERE id = ?1", [book_id], |r| {
                    r.get(0)
                })
                .ok();
            if known.is_none() {
                return Err(crate::error::not_found("book", book_id));
            }
        } else if let Some(parent_id) = input.parent_page_id.as_deref() {
            let known: Option<String> = conn
                .query_row(
                    "SELECT id FROM notes_page WHERE id = ?1",
                    [parent_id],
                    |r| r.get(0),
                )
                .ok();
            if known.is_none() {
                return Err(crate::error::not_found("note_page", parent_id));
            }
        }
        // A page below a parent keeps the parent scope even when a book is set, so its
        // siblings are the parent's children — the position index the UI shows.
        let (book_id, parent_id);
        if input.parent_page_id.is_some() {
            book_id = None;
            parent_id = input.parent_page_id.clone();
        } else {
            book_id = input.book_id.clone();
            parent_id = None;
        }
        let next: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM notes_page
             WHERE book_id IS ?1 AND parent_page_id IS ?2",
            rusqlite::params![book_id, parent_id],
            |r| r.get(0),
        )?;
        let mut page = NotePage::new(input, crate::conn::now_ms())?;
        page.book_id = book_id;
        page.parent_page_id = parent_id;
        page.position = next;
        conn.execute(
            "INSERT INTO notes_page (id, book_id, parent_page_id, title, title_norm, icon,
                                      is_template, template_key, is_pinned, position, rev,
                                      created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                page.id,
                page.book_id,
                page.parent_page_id,
                page.title,
                page.title_norm,
                page.icon,
                page.is_template as i64,
                page.template_key,
                page.is_pinned as i64,
                page.position,
                page.rev,
                page.created_at,
                page.updated_at,
            ],
        )?;
        Ok(page)
    })
}

/// Fetch one note page by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no page has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get_page(db: &Db, id: &str) -> Result<NotePage> {
    db.read(|conn| read_page(conn, id))
}

/// List note pages, newest-edited first with pinned pages keeping the top.
///
/// A `book_id` filters to that book's own pages (not nested ones — nested pages inherit
/// no book anchor, see [`create_page`]); `None` lists the whole library.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; no pages yields an empty `Vec`.
pub fn list_pages(db: &Db, book_id: Option<&str>) -> Result<Vec<NotePage>> {
    db.read(|conn| {
        let sql = match book_id {
            Some(_) => format!(
                "{PAGE_SELECT} WHERE book_id IS ?1 ORDER BY is_pinned DESC, updated_at DESC, id"
            ),
            None => format!("{PAGE_SELECT} ORDER BY is_pinned DESC, updated_at DESC, id"),
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = match book_id {
            Some(book_id) => stmt.query_map([book_id], page_from_row)?,
            None => stmt.query_map([], page_from_row)?,
        };
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit a page's content, icon and pin; its anchors and template marks stay.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no page has this id (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn update_page(db: &Db, id: &str, input: NotePageInput) -> Result<NotePage> {
    db.transact(|conn| {
        let mut page = read_page(conn, id)?;
        page.apply(input, crate::conn::now_ms())?;
        let changed = conn.execute(
            "UPDATE notes_page SET title = ?2, title_norm = ?3, icon = ?4, is_pinned = ?5,
                                   rev = ?6, updated_at = ?7
             WHERE id = ?1",
            rusqlite::params![
                page.id,
                page.title,
                page.title_norm,
                page.icon,
                page.is_pinned as i64,
                page.rev,
                page.updated_at,
            ],
        )?;
        if changed == 0 {
            return Err(crate::error::not_found("note_page", id));
        }
        Ok(page)
    })
}

/// Delete a note page; its nested pages and every block go with it (`ON DELETE CASCADE`).
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no page has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete_page(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM notes_page WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(crate::error::not_found("note_page", id));
        }
        Ok(())
    })
}

// ── Blocks ────────────────────────────────────────────────────────────────────

/// List a page's blocks in content order.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an empty page (or an unknown page —
/// the caller owns that distinction via [`get_page`]) yields an empty `Vec`.
pub fn list_blocks(db: &Db, page_id: &str) -> Result<Vec<NoteBlock>> {
    db.read(|conn| {
        let sql =
            format!("{BLOCK_SELECT} WHERE page_id = ?1 ORDER BY CAST(position AS INTEGER), id");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([page_id], block_from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Insert a block after `after_block_id`, or first when it is `None`.
///
/// Sequential-integer positions mean "insert here" renumbers the shifted side inside one
/// statement — every later block moves down by one, then the new block takes the slot.
/// An insert bumps the owning page so a page-level poll sees the content change.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the page (or `after_block_id`) is missing.
/// * [`crate::StorageError::Domain`] when `after_block_id` lives on another page, or the
///   input payload fails validation.
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn insert_block(
    db: &Db,
    page_id: &str,
    after_block_id: Option<&str>,
    input: NoteBlockInput,
) -> Result<NoteBlock> {
    db.transact(|conn| {
        read_page(conn, page_id)?;
        let now = crate::conn::now_ms();
        let slot: i64 = match after_block_id {
            None => 1,
            Some(after_id) => {
                let after = read_block(conn, after_id)?;
                if after.page_id != page_id {
                    return Err(maktabati_domain::DomainError::InvalidTransition {
                        detail: format!(
                            "cannot insert a block of page {page_id} after block \
                             {after_id}, which belongs to page {}",
                            after.page_id
                        ),
                    }
                    .into());
                }
                match after.position.parse::<i64>() {
                    Ok(pos) => pos + 1,
                    // Positions are always written by this module as decimal integers;
                    // anything else is state this file never created.
                    Err(_) => {
                        return Err(maktabati_domain::DomainError::InvalidTransition {
                            detail: format!(
                                "block {after_id} carries a non-integer position {:?}",
                                after.position
                            ),
                        }
                        .into())
                    }
                }
            }
        };
        conn.execute(
            // Two-phase shift keeps the unique index honest: SQLite enforces
            // ux_blocks_pos per statement, so a bare "+1" rewrite collides with the
            // neighbour still holding the next slot. An `x`-prefixed spelling clears
            // every moved row at once, then the plain integers are re-spelled — and all
            // positions written by this module are decimal digits, so `x` is safe.
            "UPDATE note_block
             SET position = 'x' || CAST(CAST(position AS INTEGER) + 1 AS TEXT)
             WHERE page_id = ?1 AND CAST(position AS INTEGER) >= ?2",
            rusqlite::params![page_id, slot],
        )?;
        conn.execute(
            "UPDATE note_block SET position = substr(position, 2)
             WHERE page_id = ?1 AND position LIKE 'x%'",
            rusqlite::params![page_id],
        )?;
        let block = NoteBlock::new(page_id, slot, input, now)?;
        conn.execute(
            "INSERT INTO note_block (id, page_id, parent_block_id, position, type, payload,
                                      rev, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                block.id,
                block.page_id,
                block.parent_block_id,
                block.position,
                block.block_type.as_str(),
                serde_json::to_string(&block.payload).unwrap_or_else(|_| "{}".to_owned()),
                block.rev,
                block.created_at,
                block.updated_at,
            ],
        )?;
        touch_page(conn, page_id, now)?;
        Ok(block)
    })
}

/// Edit a block's kind or payload; its position is not this function's business.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no block has this id (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails validation.
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn update_block(db: &Db, block_id: &str, input: NoteBlockInput) -> Result<NoteBlock> {
    db.transact(|conn| {
        let mut block = read_block(conn, block_id)?;
        let now = crate::conn::now_ms();
        block.apply(input, now)?;
        let changed = conn.execute(
            "UPDATE note_block SET type = ?2, payload = ?3, rev = ?4, updated_at = ?5
             WHERE id = ?1",
            rusqlite::params![
                block.id,
                block.block_type.as_str(),
                serde_json::to_string(&block.payload).unwrap_or_else(|_| "{}".to_owned()),
                block.rev,
                block.updated_at,
            ],
        )?;
        if changed == 0 {
            return Err(crate::error::not_found("note_block", block_id));
        }
        touch_page(conn, &block.page_id, now)?;
        Ok(block)
    })
}

/// Move a block to sit after `after_block_id` (`None` = first).
///
/// The page's sequence is renumbered densely around the move: the moved block leaves its
/// slot, the remaining blocks are re-spelled in order, and the block lands at its new
/// index. With at most a few hundred blocks per page the rewrite is the cheap, honest
/// plan compared to keeping a fraction-fiddled index invariant correct.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the block or list context is missing.
/// * [`crate::StorageError::Domain`] when `after_block_id` lives on another page.
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn move_block(db: &Db, block_id: &str, after_block_id: Option<&str>) -> Result<NoteBlock> {
    use maktabati_domain::DomainError;

    db.transact(|conn| {
        let block = read_block(conn, block_id)?;
        let now = crate::conn::now_ms();
        let after: Option<NoteBlock> = match after_block_id {
            None => None,
            Some(aid) => {
                let candidate = read_block(conn, aid)?;
                if candidate.page_id != block.page_id {
                    return Err(DomainError::InvalidTransition {
                        detail: format!(
                            "cannot move a block of page {} below block {aid}, \
                             which belongs to page {}",
                            block.page_id, candidate.page_id
                        ),
                    }
                    .into());
                }
                Some(candidate)
            }
        };
        let page_blocks: Vec<String> = {
            let sql =
                format!("{BLOCK_SELECT} WHERE page_id = ?1 ORDER BY CAST(position AS INTEGER), id");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([&block.page_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        }
        .into_iter()
        .filter(|id| id != block_id)
        .collect();
        let anchor = match &after {
            None => 0usize,
            Some(a) => match page_blocks.iter().position(|id| id == &a.id) {
                None => return Err(crate::error::not_found("note_block", &a.id)),
                Some(i) => i + 1,
            },
        };
        let mut ordered = page_blocks;
        ordered.insert(anchor, block_id.to_owned());
        // Same two-phase move as `insert_block`: clear every row to guarantee-unique
        // spellings first, then write the dense final ordering row by row.
        conn.execute(
            "UPDATE note_block SET position = 'x' || position WHERE page_id = ?1",
            rusqlite::params![&block.page_id],
        )?;
        for (index, id) in ordered.iter().enumerate() {
            conn.execute(
                "UPDATE note_block SET position = ?2 WHERE id = ?1",
                rusqlite::params![id, (index as i64) + 1],
            )?;
        }
        touch_page(conn, &block.page_id, now)?;
        read_block(conn, block_id)
    })
}

/// Delete a block; the page keeps the rest of its sequence (with a gap, which the order
/// by makes invisible) and its `rev`/`updated_at` move so a page-level poll notices.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no block has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete_block(db: &Db, block_id: &str) -> Result<()> {
    db.transact(|conn| {
        let block = read_block(conn, block_id)?;
        let removed = conn.execute("DELETE FROM note_block WHERE id = ?1", [block_id])?;
        if removed == 0 {
            return Err(crate::error::not_found("note_block", block_id));
        }
        touch_page(conn, &block.page_id, crate::conn::now_ms())?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageError;
    use maktabati_domain::note::limits;
    use maktabati_domain::DomainError;
    use pretty_assertions::assert_eq;

    fn test_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db
    }

    fn page_input(title: &str) -> NotePageInput {
        NotePageInput {
            title: title.to_owned(),
            ..Default::default()
        }
    }

    fn text_block(text: &str) -> NoteBlockInput {
        NoteBlockInput {
            block_type: NoteBlockType::Paragraph,
            payload: Json::from(serde_json::json!({ "text": text })),
        }
    }

    /// The block's own text member, for sequence assertions in display intent.
    fn block_text(block: &NoteBlock) -> String {
        match &block.payload {
            Json::Object(map) => match map.get("text") {
                Some(Json::String(s)) => s.clone(),
                other => format!("{other:?}"),
            },
            other => format!("{other:?}"),
        }
    }

    /// The block texts of one page, in content order — the sequence the editor renders.
    fn sequence(db: &Db, page_id: &str) -> Vec<String> {
        list_blocks(db, page_id)
            .unwrap()
            .iter()
            .map(block_text)
            .collect()
    }

    // ── pages ─────────────────────────────────────────────────────────────────

    #[test]
    fn create_get_update_round_trip_every_editable_field() {
        let db = test_db();
        let created = create_page(
            &db,
            NotePageInput {
                title: "  ملاحظات قراءة ".into(),
                icon: Some("notebook".into()),
                is_pinned: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(created.title, "ملاحظات قراءة");
        assert!(created.book_id.is_none());
        assert!(created.parent_page_id.is_none());
        assert!(created.is_pinned);
        assert_eq!(created.position, 1, "first page in its scope");
        assert_eq!(created.rev, 1);

        let got = get_page(&db, &created.id).unwrap();
        assert_eq!(got, created);

        let edited = update_page(
            &db,
            &created.id,
            NotePageInput {
                title: "عنوان جديد".into(),
                icon: None,
                is_pinned: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(edited.title, "عنوان جديد");
        assert!(edited.icon.is_none());
        assert!(!edited.is_pinned);
        assert_eq!(edited.rev, 2);
        assert_eq!(edited.created_at, created.created_at, "identity stands");
        assert!(
            edited.updated_at >= created.updated_at,
            "the edit stamps a fresh time"
        );

        // A blank title stays legal — an untitled page is a real state.
        let untitled = create_page(&db, page_input("")).unwrap();
        assert_eq!(untitled.title, "");
        assert_eq!(untitled.title_norm, "");

        // But an overlong one is refused, and the error names the cap.
        let err = create_page(
            &db,
            NotePageInput {
                title: "أ".repeat(limits::TITLE + 1),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                StorageError::Domain(DomainError::TooLong { max, .. }) if *max == limits::TITLE
            ),
            "the cap, not a bare sqlite message, is what surfaces"
        );
    }

    #[test]
    fn create_appends_position_within_its_scope() {
        let db = test_db();
        let first = create_page(&db, page_input("أول")).unwrap();
        let second = create_page(&db, page_input("ثانٍ")).unwrap();
        assert_eq!(first.position, 1);
        assert_eq!(second.position, 2, "siblings keep counting up");

        // A page nested under another rejoins the parent's children, not the roots.
        let child = create_page(
            &db,
            NotePageInput {
                parent_page_id: Some(first.id.clone()),
                ..page_input("طفل")
            },
        )
        .unwrap();
        assert_eq!(child.parent_page_id.as_deref(), Some(first.id.as_str()));
        assert_eq!(child.position, 1, "its siblings are the parent's children");

        let err = create_page(
            &db,
            NotePageInput {
                parent_page_id: Some("no-such-page".into()),
                ..page_input("تف")
            },
        )
        .unwrap_err();
        assert!(matches!(err, StorageError::NotFound { kind, .. } if kind == "note_page"));

        let err = create_page(
            &db,
            NotePageInput {
                book_id: Some("no-such-book".into()),
                ..page_input("تف")
            },
        )
        .unwrap_err();
        assert!(matches!(err, StorageError::NotFound { kind, .. } if kind == "book"));
    }

    #[test]
    fn list_orders_pinned_first_and_filters_by_book() {
        let db = test_db();
        let a = create_page(&db, page_input("أ")).unwrap();
        let _b = create_page(&db, page_input("ب")).unwrap();
        // Pin one page and edit it so it is also the newest update.
        let pinned = update_page(
            &db,
            &a.id,
            NotePageInput {
                is_pinned: true,
                ..page_input("أ")
            },
        )
        .unwrap();

        let listed = list_pages(&db, None).unwrap();
        assert!(listed[0].is_pinned, "the pinned page keeps the top");
        assert_eq!(listed.len(), 2);

        // A book filter is honest both ways: an unknown book lists nothing (not everything),
        // and the book-anchored page shows only under its own book.
        assert!(list_pages(&db, Some("no-such-book")).unwrap().is_empty());

        let credit = maktabati_domain::book::AuthorCredit {
            author_id: crate::repo::author::create(&db, page_input_author())
                .unwrap()
                .id,
            role: maktabati_domain::AuthorRole::Author,
        };
        let _language = crate::repo::language::create(
            &db,
            maktabati_domain::LanguageInput {
                code: "ar".into(),
                name_ar: "العربية".into(),
                name_en: "Arabic".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let book = crate::repo::book::create(
            &db,
            maktabati_domain::BookInput {
                title: "كتاب".into(),
                language_code: Some("ar".into()),
                authors: vec![credit],
                ..Default::default()
            },
        )
        .unwrap();
        let anchored = create_page(
            &db,
            NotePageInput {
                book_id: Some(book.id.clone()),
                ..page_input("دليل الكتاب")
            },
        )
        .unwrap();
        let for_book = list_pages(&db, Some(&book.id)).unwrap();
        assert_eq!(
            for_book.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            [anchored.id.as_str()]
        );
        assert_eq!(for_book[0].book_id.as_deref(), Some(book.id.as_str()));
        assert!(
            pinned.is_pinned && pinned.book_id.is_none(),
            "root and book-anchored pages are different scopes"
        );
    }

    fn page_input_author() -> maktabati_domain::AuthorInput {
        maktabati_domain::AuthorInput {
            name: "نحوك".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn delete_removes_the_page_and_cascades_its_blocks() {
        let db = test_db();
        let page = create_page(&db, page_input("x")).unwrap();
        insert_block(&db, &page.id, None, text_block("بند")).unwrap();

        delete_page(&db, &page.id).unwrap();
        assert!(matches!(
            get_page(&db, &page.id).unwrap_err(),
            StorageError::NotFound { .. }
        ));
        assert!(list_blocks(&db, &page.id).unwrap().is_empty());

        let err = delete_page(&db, &page.id).unwrap_err();
        assert!(matches!(err, StorageError::NotFound { .. }));
    }

    // ── blocks ────────────────────────────────────────────────────────────────

    #[test]
    fn blocks_insert_update_and_read_in_order() {
        let db = test_db();
        let page = create_page(&db, page_input("صفحة")).unwrap();
        let rev_after = |db: &Db, page_id: &str| get_page(db, page_id).unwrap().rev;

        let one = insert_block(&db, &page.id, None, text_block("أولاً")).unwrap();
        assert_eq!(one.position, "1");
        let two = insert_block(&db, &page.id, Some(&one.id), text_block("ثانياً")).unwrap();
        assert_eq!(two.position, "2", "the anchor's next slot");
        // Inserting first pushes every existing block down.
        let zero = insert_block(&db, &page.id, None, text_block("مقدّمة")).unwrap();
        assert_eq!(zero.position, "1");

        assert_eq!(
            sequence(&db, &page.id),
            ["مقدّمة", "أولاً", "ثانياً"],
            "the stored sequence is read back in order"
        );
        assert!(
            rev_after(&db, &page.id) >= 4,
            "each block write touched the page"
        );
        let rev_before = rev_after(&db, &page.id);
        let edited = update_block(&db, &two.id, text_block("ثانياً معدّلة")).unwrap();
        assert_eq!(edited.rev, 2);
        assert_eq!(
            get_page(&db, &page.id).unwrap().rev,
            rev_before + 1,
            "the page noticed the content change"
        );

        // A deleted block leaves its siblings' relative order intact.
        delete_block(&db, &one.id).unwrap();
        assert_eq!(sequence(&db, &page.id), ["مقدّمة", "ثانياً معدّلة"]);
    }

    #[test]
    fn insert_refuses_foreign_and_missing_anchor_blocks() {
        let db = test_db();
        let p1 = create_page(&db, page_input("١")).unwrap();
        let p2 = create_page(&db, page_input("٢")).unwrap();
        let foreign = insert_block(&db, &p2.id, None, text_block("لك")).unwrap();

        let err = insert_block(&db, &p1.id, Some(&foreign.id), text_block("ها")).unwrap_err();
        assert!(
            matches!(&err, StorageError::Domain(e)
                if matches!(e, DomainError::InvalidTransition { detail }
                    if detail.contains("belongs to page"))),
            "cross-page anchors are a caller mistake, not a missing row"
        );

        let err = insert_block(&db, &p1.id, Some("no-such-block"), text_block("ها")).unwrap_err();
        assert!(matches!(err, StorageError::NotFound { kind, .. } if kind == "note_block"));

        // Payload validation surfaces from the repo too.
        let err = insert_block(
            &db,
            &p1.id,
            None,
            NoteBlockInput {
                block_type: NoteBlockType::Paragraph,
                payload: Json::String("bare text".into()),
            },
        )
        .unwrap_err();
        assert!(matches!(err, StorageError::Domain(_)));
    }

    #[test]
    fn move_block_renumbers_the_sequence_to_the_new_anchor() {
        let db = test_db();
        let page = create_page(&db, page_input("عمود")).unwrap();
        let a = insert_block(&db, &page.id, None, text_block("١")).unwrap();
        let b = insert_block(&db, &page.id, Some(&a.id), text_block("٢")).unwrap();
        let c = insert_block(&db, &page.id, Some(&b.id), text_block("٣")).unwrap();
        assert_eq!(sequence(&db, &page.id), ["١", "٢", "٣"]);

        // Move the head after the tail → [٢، ٣، ١]
        let a = move_block(&db, &a.id, Some(&c.id)).unwrap();
        assert_eq!(sequence(&db, &page.id), ["٢", "٣", "١"]);

        // And back to the top → [١، ٢، ٣]
        move_block(&db, &a.id, None).unwrap();
        assert_eq!(sequence(&db, &page.id), ["١", "٢", "٣"]);

        // Positions stay unique and contiguous after the dance (the unique index held).
        let positions: Vec<i64> = list_blocks(&db, &page.id)
            .unwrap()
            .iter()
            .filter_map(|b| b.position.parse().ok())
            .collect();
        assert_eq!(positions, [1, 2, 3]);

        let err = move_block(&db, &a.id, Some("no-such-block")).unwrap_err();
        assert!(matches!(err, StorageError::NotFound { kind, .. } if kind == "note_block"));
    }
}
