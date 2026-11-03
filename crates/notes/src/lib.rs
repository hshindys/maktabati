//! Notes export codecs.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};

use maktabati_storage::{Db, Result};
use serde_json::Value;

struct Page {
    id: String,
    parent_id: Option<String>,
    title: String,
    book_title: Option<String>,
}

struct Block {
    id: String,
    parent_id: Option<String>,
    kind: String,
    payload: Value,
}

/// Render all note pages and blocks as one Markdown document.
///
/// Page hierarchy is preserved as nested headings; block types with direct Markdown
/// equivalents are rendered semantically, while unsupported rich blocks retain their text.
///
/// # Errors
///
/// Returns a storage error when the notes cannot be read or a stored block payload is malformed.
pub fn export_markdown(db: &Db) -> Result<String> {
    let (pages, blocks) = db.read(|connection| {
        let mut pages = BTreeMap::new();
        {
            let mut statement = connection.prepare(
                "SELECT np.id, np.parent_page_id, np.title, b.title
                 FROM notes_page np
                 LEFT JOIN books b ON b.id = np.book_id
                 ORDER BY np.position, np.id",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(Page {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    title: row.get(2)?,
                    book_title: row.get(3)?,
                })
            })?;
            for row in rows {
                let page = row?;
                pages.insert(page.id.clone(), page);
            }
        }

        let mut blocks: BTreeMap<String, Vec<Block>> = BTreeMap::new();
        {
            let mut statement = connection.prepare(
                "SELECT id, page_id, parent_block_id, type, payload
                 FROM note_block
                 ORDER BY page_id, position, id",
            )?;
            let rows = statement.query_map([], |row| {
                let payload: String = row.get(4)?;
                let payload = serde_json::from_str(&payload).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok((
                    row.get::<_, String>(1)?,
                    Block {
                        id: row.get(0)?,
                        parent_id: row.get(2)?,
                        kind: row.get(3)?,
                        payload,
                    },
                ))
            })?;
            for row in rows {
                let (page_id, block) = row?;
                blocks.entry(page_id).or_default().push(block);
            }
        }
        Ok((pages, blocks))
    })?;

    let mut children: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    for page in pages.values() {
        children
            .entry(page.parent_id.clone())
            .or_default()
            .push(page.id.clone());
    }

    let mut markdown = String::new();
    let mut visited = HashSet::new();
    if let Some(roots) = children.get(&None) {
        for id in roots {
            render_page(
                id,
                0,
                &pages,
                &children,
                &blocks,
                &mut visited,
                &mut markdown,
            );
        }
    }
    for id in pages.keys() {
        if !visited.contains(id) {
            render_page(
                id,
                0,
                &pages,
                &children,
                &blocks,
                &mut visited,
                &mut markdown,
            );
        }
    }
    Ok(markdown)
}

fn render_page(
    id: &str,
    depth: usize,
    pages: &BTreeMap<String, Page>,
    children: &BTreeMap<Option<String>, Vec<String>>,
    blocks: &BTreeMap<String, Vec<Block>>,
    visited: &mut HashSet<String>,
    markdown: &mut String,
) {
    if !visited.insert(id.to_owned()) {
        return;
    }
    let Some(page) = pages.get(id) else {
        return;
    };

    let heading_level = depth.saturating_add(1).min(6);
    markdown.push_str(&"#".repeat(heading_level));
    markdown.push(' ');
    markdown.push_str(&page.title);
    markdown.push_str("\n\n");
    if let Some(book_title) = page.book_title.as_deref() {
        markdown.push_str("> ");
        markdown.push_str(book_title);
        markdown.push_str("\n\n");
    }
    if let Some(page_blocks) = blocks.get(id) {
        let block_depths = block_depths(page_blocks);
        for (index, block) in page_blocks.iter().enumerate() {
            let depth = block_depths.get(index).copied().unwrap_or(0);
            render_block(block, depth, markdown);
        }
    }
    markdown.push('\n');

    if let Some(child_ids) = children.get(&Some(id.to_owned())) {
        for child_id in child_ids {
            render_page(
                child_id,
                depth.saturating_add(1),
                pages,
                children,
                blocks,
                visited,
                markdown,
            );
        }
    }
}

fn block_depths(blocks: &[Block]) -> Vec<usize> {
    let ids: Vec<&str> = blocks.iter().map(|block| block.id.as_str()).collect();
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let mut depth = 0usize;
            let mut parent = block.parent_id.as_deref();
            let mut visited = HashSet::new();
            while let Some(parent_id) = parent {
                if !visited.insert(parent_id) {
                    break;
                }
                let Some(parent_index) = ids.iter().position(|candidate| *candidate == parent_id)
                else {
                    break;
                };
                depth = depth.saturating_add(1);
                parent = blocks[parent_index].parent_id.as_deref();
                if parent_index == index {
                    break;
                }
            }
            depth.min(8)
        })
        .collect()
}

fn render_block(block: &Block, depth: usize, markdown: &mut String) {
    let text = block
        .payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let indent = "  ".repeat(depth);
    match block.kind.as_str() {
        "heading_1" | "heading_2" | "heading_3" => {
            let level = match block.kind.as_str() {
                "heading_1" => 2,
                "heading_2" => 3,
                _ => 4,
            };
            markdown.push_str(&"#".repeat(level));
            markdown.push(' ');
            markdown.push_str(text);
            markdown.push_str("\n\n");
        }
        "bulleted_list" => {
            markdown.push_str(&indent);
            markdown.push_str("- ");
            markdown.push_str(text);
            markdown.push('\n');
        }
        "numbered_list" => {
            markdown.push_str(&indent);
            markdown.push_str("1. ");
            markdown.push_str(text);
            markdown.push('\n');
        }
        "todo" => {
            let checked = block
                .payload
                .get("checked")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            markdown.push_str(&indent);
            markdown.push_str(if checked { "- [x] " } else { "- [ ] " });
            markdown.push_str(text);
            markdown.push('\n');
        }
        "quote" => {
            markdown.push_str("> ");
            markdown.push_str(text);
            markdown.push_str("\n\n");
        }
        "code" => {
            let longest = text
                .split('\n')
                .map(|line| {
                    line.chars()
                        .take_while(|character| *character == '`')
                        .count()
                })
                .max()
                .unwrap_or(0);
            let fence = "`".repeat(longest.saturating_add(1).max(3));
            markdown.push_str(&fence);
            markdown.push('\n');
            markdown.push_str(text);
            markdown.push('\n');
            markdown.push_str(&fence);
            markdown.push_str("\n\n");
        }
        "divider" => markdown.push_str("---\n\n"),
        _ if !text.is_empty() => {
            markdown.push_str(&indent);
            markdown.push_str(text);
            markdown.push_str("\n\n");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_storage::{migrate, Db};

    #[test]
    fn exports_nested_pages_and_block_markdown() {
        let directory = tempfile::tempdir().unwrap();
        let db = Db::open(&directory.path().join("notes.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.transact(|connection| {
            connection.execute(
                "INSERT INTO notes_page (id, title, title_norm, position, created_at, updated_at)
                 VALUES ('page-1', 'Reading notes', 'reading notes', 1, 1, 1)",
                [],
            )?;
            connection.execute(
                "INSERT INTO notes_page (id, parent_page_id, title, title_norm, position, created_at, updated_at)
                 VALUES ('page-2', 'page-1', 'Quotes', 'quotes', 1, 1, 1)",
                [],
            )?;
            connection.execute(
                "INSERT INTO note_block (id, page_id, position, type, payload, created_at, updated_at)
                 VALUES ('block-1', 'page-1', '1', 'heading_1', '{\"text\":\"Key ideas\"}', 1, 1)",
                [],
            )?;
            connection.execute(
                "INSERT INTO note_block (id, page_id, parent_block_id, position, type, payload, created_at, updated_at)
                 VALUES ('block-2', 'page-1', 'block-1', '2', 'bulleted_list', '{\"text\":\"One idea\"}', 1, 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        let markdown = export_markdown(&db).unwrap();

        assert!(markdown.contains("# Reading notes"));
        assert!(markdown.contains("## Key ideas"));
        assert!(markdown.contains("  - One idea"));
        assert!(markdown.contains("## Quotes"));
    }
}
