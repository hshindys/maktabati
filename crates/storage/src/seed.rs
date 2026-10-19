//! First-run seed data: languages, the bilingual category taxonomy and the default rubric.
//!
//! Seeded in **Rust rather than SQL** on purpose: `name_norm` must come from
//! `maktabati_domain::text::normalize` so the seed is guaranteed to produce exactly the same
//! normalized keys as user-created rows. (Hand-writing those literals into SQL would be a second
//! source of truth for the normalization contract — precisely the pitfall recorded in
//! `docs/00-research.md` §4.7.)
//!
//! [`apply`] is **idempotent**: running it on an already-seeded database changes nothing, so a
//! fresh install and an upgraded install converge to the same state (`docs/03-data-model.md` §7).

use rusqlite::Connection;

use maktabati_domain::category::{new_category, CategoryInput};
use maktabati_domain::{Category, Direction};

use crate::conn::now_ms;
use crate::{Result, StorageError};

struct SeedLang {
    code: &'static str,
    name_ar: &'static str,
    name_en: &'static str,
    direction: Direction,
}

const LANGUAGES: &[SeedLang] = &[
    SeedLang {
        code: "ar",
        name_ar: "العربية",
        name_en: "Arabic",
        direction: Direction::Rtl,
    },
    SeedLang {
        code: "en",
        name_ar: "الإنجليزية",
        name_en: "English",
        direction: Direction::Ltr,
    },
    SeedLang {
        code: "fr",
        name_ar: "الفرنسية",
        name_en: "French",
        direction: Direction::Ltr,
    },
    SeedLang {
        code: "de",
        name_ar: "الألمانية",
        name_en: "German",
        direction: Direction::Ltr,
    },
    SeedLang {
        code: "es",
        name_ar: "الإسبانية",
        name_en: "Spanish",
        direction: Direction::Ltr,
    },
    SeedLang {
        code: "tr",
        name_ar: "التركية",
        name_en: "Turkish",
        direction: Direction::Ltr,
    },
    SeedLang {
        code: "fa",
        name_ar: "الفارسية",
        name_en: "Persian",
        direction: Direction::Rtl,
    },
    SeedLang {
        code: "ur",
        name_ar: "الأردية",
        name_en: "Urdu",
        direction: Direction::Rtl,
    },
];

/// A category node. `path` is the materialized path of `key`s, so every entry's parent is
/// **guaranteed to precede it** in this array (roots first) — see [`CATEGORIES`].
struct SeedCat {
    key: &'static str,
    path: &'static str,
    name_ar: &'static str,
    name_en: &'static str,
}

/// The seeded bilingual taxonomy — `docs/03-data-model.md` §8.
/// Ordering invariant: a child always appears after its parent.
const CATEGORIES: &[SeedCat] = &[
    SeedCat {
        key: "literature",
        path: "/literature",
        name_ar: "الأدب",
        name_en: "Literature",
    },
    SeedCat {
        key: "novels",
        path: "/literature/novels",
        name_ar: "روايات",
        name_en: "Novels",
    },
    SeedCat {
        key: "fantasy",
        path: "/literature/novels/fantasy",
        name_ar: "فانتازيا",
        name_en: "Fantasy",
    },
    SeedCat {
        key: "science_fiction",
        path: "/literature/novels/science_fiction",
        name_ar: "خيال علمي",
        name_en: "Science Fiction",
    },
    SeedCat {
        key: "poetry",
        path: "/literature/poetry",
        name_ar: "شعر",
        name_en: "Poetry",
    },
    SeedCat {
        key: "drama",
        path: "/literature/drama",
        name_ar: "مسرح",
        name_en: "Drama",
    },
    SeedCat {
        key: "islam",
        path: "/islam",
        name_ar: "الإسلام",
        name_en: "Islam",
    },
    SeedCat {
        key: "islamic_sciences",
        path: "/islam/islamic_sciences",
        name_ar: "علوم شرعية",
        name_en: "Islamic Sciences",
    },
    SeedCat {
        key: "fiqh",
        path: "/islam/islamic_sciences/fiqh",
        name_ar: "فقه",
        name_en: "Fiqh",
    },
    SeedCat {
        key: "tafsir",
        path: "/islam/islamic_sciences/tafsir",
        name_ar: "تفسير",
        name_en: "Tafsir",
    },
    SeedCat {
        key: "hadith",
        path: "/islam/islamic_sciences/hadith",
        name_ar: "حديث",
        name_en: "Hadith",
    },
    SeedCat {
        key: "sirah",
        path: "/islam/islamic_sciences/sirah",
        name_ar: "سيرة",
        name_en: "Sirah",
    },
    SeedCat {
        key: "creed",
        path: "/islam/creed",
        name_ar: "عقيدة",
        name_en: "Creed",
    },
    SeedCat {
        key: "duas",
        path: "/islam/duas",
        name_ar: "أدعية وأذكار",
        name_en: "Duas & Adhkar",
    },
    SeedCat {
        key: "history",
        path: "/history",
        name_ar: "التاريخ",
        name_en: "History",
    },
    SeedCat {
        key: "history_egypt",
        path: "/history/history_egypt",
        name_ar: "تاريخ مصر",
        name_en: "History of Egypt",
    },
    SeedCat {
        key: "history_arab",
        path: "/history/history_arab",
        name_ar: "تاريخ العرب",
        name_en: "Arab History",
    },
    SeedCat {
        key: "history_world",
        path: "/history/history_world",
        name_ar: "تاريخ العالم",
        name_en: "World History",
    },
    SeedCat {
        key: "philosophy",
        path: "/philosophy",
        name_ar: "الفلسافة",
        name_en: "Philosophy",
    },
    SeedCat {
        key: "logic",
        path: "/philosophy/logic",
        name_ar: "منطق",
        name_en: "Logic",
    },
    SeedCat {
        key: "science",
        path: "/science",
        name_ar: "العلوم",
        name_en: "Science",
    },
    SeedCat {
        key: "physics",
        path: "/science/physics",
        name_ar: "فيزياء",
        name_en: "Physics",
    },
    SeedCat {
        key: "chemistry",
        path: "/science/chemistry",
        name_ar: "كيمياء",
        name_en: "Chemistry",
    },
    SeedCat {
        key: "mathematics",
        path: "/science/mathematics",
        name_ar: "رياضيات",
        name_en: "Mathematics",
    },
    SeedCat {
        key: "biology",
        path: "/science/biology",
        name_ar: "بيولوجيا",
        name_en: "Biology",
    },
    SeedCat {
        key: "technology",
        path: "/technology",
        name_ar: "التقنية",
        name_en: "Technology",
    },
    SeedCat {
        key: "programming",
        path: "/technology/programming",
        name_ar: "برمجة",
        name_en: "Programming",
    },
    SeedCat {
        key: "software_engineering",
        path: "/technology/programming/software_engineering",
        name_ar: "هندسة برمجيات",
        name_en: "Software Engineering",
    },
    SeedCat {
        key: "data",
        path: "/technology/data",
        name_ar: "بيانات",
        name_en: "Data",
    },
    SeedCat {
        key: "cybersecurity",
        path: "/technology/cybersecurity",
        name_ar: "أمن سيبراني",
        name_en: "Cybersecurity",
    },
    SeedCat {
        key: "self_development",
        path: "/self_development",
        name_ar: "التطوير الذاتي",
        name_en: "Self-Development",
    },
    SeedCat {
        key: "habits",
        path: "/self_development/habits",
        name_ar: "عادات",
        name_en: "Habits",
    },
    SeedCat {
        key: "productivity",
        path: "/self_development/productivity",
        name_ar: "إنتاجية",
        name_en: "Productivity",
    },
    SeedCat {
        key: "biography",
        path: "/biography",
        name_ar: "السيرة الذاتية",
        name_en: "Biography",
    },
    SeedCat {
        key: "scholars",
        path: "/biography/scholars",
        name_ar: "علماء",
        name_en: "Scholars",
    },
    SeedCat {
        key: "writers",
        path: "/biography/writers",
        name_ar: "أدباء",
        name_en: "Writers",
    },
    SeedCat {
        key: "children",
        path: "/children",
        name_ar: "الأطفال",
        name_en: "Children",
    },
    SeedCat {
        key: "children_stories",
        path: "/children/children_stories",
        name_ar: "قصص أطفال",
        name_en: "Children's Stories",
    },
    SeedCat {
        key: "educational",
        path: "/children/educational",
        name_ar: "تعليمي",
        name_en: "Educational",
    },
    SeedCat {
        key: "arts",
        path: "/arts",
        name_ar: "الفنون",
        name_en: "Arts",
    },
    SeedCat {
        key: "photography",
        path: "/arts/photography",
        name_ar: "تصوير",
        name_en: "Photography",
    },
    SeedCat {
        key: "design",
        path: "/arts/design",
        name_ar: "تصميم",
        name_en: "Design",
    },
    SeedCat {
        key: "economics",
        path: "/economics",
        name_ar: "الاقتصاد",
        name_en: "Economics",
    },
    SeedCat {
        key: "finance",
        path: "/economics/finance",
        name_ar: "مالية",
        name_en: "Finance",
    },
    SeedCat {
        key: "medicine",
        path: "/medicine",
        name_ar: "الطب",
        name_en: "Medicine",
    },
    SeedCat {
        key: "medicine_general",
        path: "/medicine/medicine_general",
        name_ar: "عام",
        name_en: "General",
    },
    SeedCat {
        key: "law",
        path: "/law",
        name_ar: "القانون",
        name_en: "Law",
    },
    SeedCat {
        key: "politics",
        path: "/politics",
        name_ar: "السياسة",
        name_en: "Politics",
    },
    SeedCat {
        key: "travel",
        path: "/travel",
        name_ar: "الرحلات",
        name_en: "Travel",
    },
    SeedCat {
        key: "cooking",
        path: "/cooking",
        name_ar: "الطبخ",
        name_en: "Cooking",
    },
    SeedCat {
        key: "sports",
        path: "/sports",
        name_ar: "الرياضة",
        name_en: "Sports",
    },
];

/// Number of root categories documented in `docs/03-data-model.md` §8.
const EXPECTED_ROOTS: i64 = 17;

/// Default rubric criteria — `docs/03-data-model.md` §8.
const CRITERIA: &[(&str, &str, &str)] = &[
    ("content_depth", "عمق المحتوى", "Content depth"),
    ("writing_style", "أسلوب الكتابة", "Writing style"),
    ("editing", "التدقيق والتحرير", "Proofreading & editing"),
    (
        "print_quality",
        "جودة الطباعة والورق",
        "Print & paper quality",
    ),
    ("translation", "جودة الترجمة", "Translation quality"),
    ("value_for_price", "جودة مقابل السعر", "Value for price"),
    (
        "structure",
        "تنظيم وهيكلة المحتوى",
        "Structure & organization",
    ),
    ("references", "المراجع والمصادر", "References & sources"),
];

/// Insert seed rows that are not yet present. Safe to call on every startup.
///
/// # Errors
///
/// * [`StorageError::Sqlite`] on any statement failure.
/// * [`StorageError::Migration`] when the taxonomy itself is inconsistent (a child listed
///   before its parent, or an unknown parent path) — a programming error in the seed table.
pub fn apply(conn: &Connection) -> Result<()> {
    let now = now_ms();
    seed_languages(conn)?;
    seed_categories(conn, now)?;
    seed_criteria(conn, now)?;
    Ok(())
}

fn seed_languages(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO languages (code, name_ar, name_en, direction, is_active, position)
         VALUES (?1, ?2, ?3, ?4, 1, ?5)
         ON CONFLICT(code) DO NOTHING",
    )?;
    for (i, l) in LANGUAGES.iter().enumerate() {
        stmt.execute(rusqlite::params![
            l.code,
            l.name_ar,
            l.name_en,
            l.direction.as_str(),
            i as i64
        ])?;
    }
    tracing::debug!(count = LANGUAGES.len(), "languages seeded");
    Ok(())
}

/// Read a category back by its materialized path so `new_category` can compute the child's
/// path and depth from its real parent (rather than re-implementing that logic here).
fn category_by_path(conn: &Connection, path: &str) -> Result<Option<Category>> {
    let row = conn.query_row(
        "SELECT id, parent_id, key, path, depth, position
         FROM categories WHERE path = ?1",
        [path],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        },
    );
    match row {
        Ok((id, parent_id, key, ppath, depth, position)) => Ok(Some(Category {
            id,
            parent_id,
            key,
            name_ar: String::new(),
            name_en: String::new(),
            name_norm: String::new(),
            path: ppath,
            depth: depth as usize,
            position,
            icon: None,
            color: None,
            is_system: true,
            created_at: 0,
            updated_at: 0,
        })),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn seed_categories(conn: &Connection, now: i64) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO categories
           (id, parent_id, key, name_ar, name_en, name_norm, path, depth,
            position, icon, color, is_system, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,1,?12,?13)
         ON CONFLICT(path) DO NOTHING",
    )?;

    let mut roots_seen: i64 = 0;
    let mut siblings = std::collections::HashMap::<String, i64>::new();

    for c in CATEGORIES {
        let parent_path = match c.path.rfind('/') {
            Some(idx) if idx > 0 => Some(&c.path[..idx]),
            _ => None,
        };

        let parent = match parent_path {
            None => {
                roots_seen += 1;
                None
            }
            Some(pp) => match category_by_path(conn, pp)? {
                Some(p) => Some(p),
                None => {
                    return Err(StorageError::Migration(format!(
                    "seeded category `{}` declares parent `{}` which appears later (or not at all)",
                    c.path, pp
                )))
                }
            },
        };

        let sibling_key = parent_path.unwrap_or("").to_owned();
        let position = *siblings.get(&sibling_key).unwrap_or(&0);
        siblings.insert(sibling_key, position + 1);

        let cat = new_category(
            CategoryInput {
                key: c.key.to_owned(),
                name_ar: c.name_ar.to_owned(),
                name_en: c.name_en.to_owned(),
                icon: None,
                color: None,
            },
            parent.as_ref(),
            position,
            now,
            true,
        )?;

        stmt.execute(rusqlite::params![
            cat.id,
            cat.parent_id,
            cat.key,
            cat.name_ar,
            cat.name_en,
            cat.name_norm,
            cat.path,
            cat.depth as i64,
            cat.position,
            cat.icon,
            cat.color,
            cat.created_at,
            cat.updated_at
        ])?;
    }

    assert_roots(roots_seen)?;
    tracing::debug!(
        count = CATEGORIES.len(),
        roots = roots_seen,
        "categories seeded"
    );
    Ok(())
}

fn assert_roots(seen: i64) -> Result<()> {
    if seen != EXPECTED_ROOTS {
        return Err(StorageError::Migration(format!(
            "taxonomy has {seen} root categories, expected {EXPECTED_ROOTS} — update \
             EXPECTED_ROOTS and docs/03-data-model.md together if this is intentional"
        )));
    }
    Ok(())
}

fn seed_criteria(conn: &Connection, now: i64) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO rating_criteria
           (id, key, name_ar, name_en, description_ar, description_en, weight, position,
            is_active, is_system, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL, 1.0, ?5, 1, 1, ?6, ?6)
         ON CONFLICT(key) DO NOTHING",
    )?;
    for (i, (key, ar, en)) in CRITERIA.iter().enumerate() {
        stmt.execute(rusqlite::params![
            maktabati_domain::ids::new_id(),
            key,
            ar,
            en,
            i as i64,
            now
        ])?;
    }
    tracing::debug!(count = CRITERIA.len(), "rating criteria seeded");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conn::Db;
    use crate::migrate;
    use pretty_assertions::assert_eq;

    fn seeded_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(apply).unwrap();
        db
    }

    #[test]
    fn seeds_expected_counts() {
        let db = seeded_db();
        db.read(|c| {
            let langs: i64 = c.query_row("SELECT COUNT(*) FROM languages", [], |r| r.get(0))?;
            assert_eq!(langs, LANGUAGES.len() as i64);

            let cats: i64 = c.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;
            assert_eq!(cats, CATEGORIES.len() as i64);

            let crit: i64 =
                c.query_row("SELECT COUNT(*) FROM rating_criteria", [], |r| r.get(0))?;
            assert_eq!(crit, CRITERIA.len() as i64);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn seeding_is_idempotent() {
        let db = seeded_db();
        let count = |db: &Db| {
            db.read(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM categories", [], |r| {
                    r.get::<_, i64>(0)
                })?)
            })
            .unwrap()
        };
        let before = count(&db);
        db.write(apply).unwrap();
        db.write(apply).unwrap();
        assert_eq!(before, count(&db));
    }

    #[test]
    fn category_paths_depths_and_normalized_names_are_correct() {
        let db = seeded_db();
        db.read(|c| {
            let fantasy: (String, i64, String) = c.query_row(
                "SELECT path, depth, name_norm FROM categories WHERE key = 'fantasy'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            assert_eq!(fantasy.0, "/literature/novels/fantasy");
            assert_eq!(fantasy.1, 2);
            assert_eq!(fantasy.2, maktabati_domain::text::normalize("فانتازيا"));

            let roots: i64 = c.query_row(
                "SELECT COUNT(*) FROM categories WHERE parent_id IS NULL",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(roots, EXPECTED_ROOTS);

            let orphans: i64 = c.query_row(
                "SELECT COUNT(*) FROM categories ch
                 WHERE ch.parent_id IS NOT NULL
                   AND NOT EXISTS (SELECT 1 FROM categories pa WHERE pa.id = ch.parent_id)",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(orphans, 0);

            // Every child is exactly one level deeper than its parent.
            let bad_depth: i64 = c.query_row(
                "SELECT COUNT(*) FROM categories ch
                 JOIN categories pa ON pa.id = ch.parent_id
                 WHERE ch.depth <> pa.depth + 1",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(bad_depth, 0);

            // name_norm must equal normalize(name_ar) for every seeded row.
            let mut stmt =
                c.prepare("SELECT name_ar, name_norm FROM categories WHERE parent_id IS NULL")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (ar, norm) = row?;
                assert_eq!(norm, maktabati_domain::text::normalize(&ar), "row: {ar}");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn language_directions_are_right() {
        let db = seeded_db();
        db.read(|c| {
            let rtl: i64 = c.query_row(
                "SELECT COUNT(*) FROM languages WHERE direction = 'rtl'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(rtl, 3, "ar, fa and ur are RTL");
            let ar: String = c.query_row(
                "SELECT direction FROM languages WHERE code = 'ar'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(ar, "rtl");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn seeded_criteria_match_the_documented_rubric() {
        let db = seeded_db();
        db.read(|c| {
            let mut stmt =
                c.prepare("SELECT key, name_ar, name_en FROM rating_criteria ORDER BY position")?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            let got: Vec<_> = rows.collect::<std::result::Result<_, _>>()?;
            let want: Vec<_> = CRITERIA
                .iter()
                .map(|(k, a, e)| (k.to_string(), a.to_string(), e.to_string()))
                .collect();
            assert_eq!(got, want);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn taxonomy_array_is_self_consistent() {
        // Children always appear after their parent, so seeding in array order works.
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for c in CATEGORIES {
            let parent = match c.path.rfind('/') {
                Some(idx) if idx > 0 => &c.path[..idx],
                _ => "",
            };
            if !parent.is_empty() {
                assert!(
                    seen.contains(parent),
                    "`{}` appears before its parent `{}`",
                    c.path,
                    parent
                );
            }
            assert!(seen.insert(c.path), "duplicate path {}", c.path);
        }
    }
}
