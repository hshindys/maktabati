//! Every stored enumeration.
//!
//! Each variant's `serde` representation is the exact string used by the SQL `CHECK`
//! constraints in `crates/storage/migrations/0001_init.sql`. A test in `storage` asserts that
//! every Rust variant appears in the DDL, so the two can never silently diverge.

use serde::{Deserialize, Serialize};
use specta::Type;

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $sql:literal,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $(#[serde(rename = $sql)] $(#[$vmeta])* $variant,)*
        }

        impl $name {
            /// Every variant, in declaration order (used by the DDL-conformance test).
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// The literal stored in SQLite.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $sql,)* }
            }

            /// Parse the literal stored in SQLite.
            ///
            /// Returns `None` for unknown values (forward compatibility: a newer database may
            /// contain a variant this build does not know).
            #[must_use]
            pub fn from_db(s: &str) -> Option<Self> {
                match s { $($sql => Some(Self::$variant),)* _ => None }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

string_enum! {
    /// Current reading status of a book (the brief: to-read / reading / finished / paused /
    /// abandoned — Hard Cover's "did not finish" maps to `Abandoned`).
    ReadingStatus {
        ToRead => "to_read",
        Reading => "reading",
        Finished => "finished",
        Paused => "paused",
        Abandoned => "abandoned",
    }
}

string_enum! {
    /// Physical or digital format of a copy.
    BookFormat {
        Print => "print",
        Ebook => "ebook",
        Audiobook => "audiobook",
        Pdf => "pdf",
        Other => "other",
    }
}

string_enum! {
    /// Condition of a physical copy (collector persona).
    Condition {
        New => "new",
        LikeNew => "like_new",
        VeryGood => "very_good",
        Good => "good",
        Fair => "fair",
        Poor => "poor",
    }
}

string_enum! {
    /// Role an author plays on a book (Calibre-style multi-role credits).
    AuthorRole {
        Author => "author",
        Translator => "translator",
        Editor => "editor",
        Illustrator => "illustrator",
        Foreword => "foreword",
        Other => "other",
    }
}

string_enum! {
    /// Where a book record came from (used for provenance badges and import auditing).
    BookSource {
        Manual => "manual",
        Isbn => "isbn",
        FolderScan => "folder_scan",
        Csv => "csv",
        Goodreads => "goodreads",
        Calibre => "calibre",
        Json => "json",
    }
}

string_enum! {
    /// Where a cover image came from.
    CoverSource {
        Manual => "manual",
        OpenLibrary => "openlibrary",
        GoogleBooks => "googlebooks",
        Embedded => "embedded",
        Generated => "generated",
    }
}

string_enum! {
    /// How an attachment is stored.
    StorageMode {
        /// Copied into the library, content-addressed by SHA-256.
        Copy => "copy",
        /// Left where the user put it; only the path and hash are recorded.
        Reference => "reference",
    }
}

string_enum! {
    /// Attachment file kind.
    FileKind {
        Epub => "epub",
        Pdf => "pdf",
        Mobi => "mobi",
        Azw3 => "azw3",
        Txt => "txt",
        Audio => "audio",
        Other => "other",
    }
}

string_enum! {
    /// Shelf type: explicit membership vs a saved filter.
    ShelfKind {
        Static => "static",
        Smart => "smart",
    }
}

string_enum! {
    /// Type of a custom (user-defined) column, Calibre-style.
    CustomColumnType {
        Text => "text",
        Integer => "integer",
        Float => "float",
        Bool => "bool",
        Date => "date",
        Enum => "enum",
        Rating => "rating",
    }
}

string_enum! {
    /// Writing direction of a language.
    Direction {
        Rtl => "rtl",
        Ltr => "ltr",
    }
}

string_enum! {
    /// Highlight colour used by the reader and by quotes.
    QuoteColor {
        Yellow => "yellow",
        Green => "green",
        Blue => "blue",
        Pink => "pink",
        Red => "red",
        Purple => "purple",
    }
}

string_enum! {
    /// A saved-filter shelf's or palette's free-text + structured filter version.
    /// (Kept here so the serialization contract has one home.)
    QueryVersion {
        V1 => "v1",
    }
}

string_enum! {
    /// The kind of a note block (كل أنواع `note_block` في القيد الست عشري للمخطط).
    ///
    /// The v1 editor surfaces the text kinds (paragraph, headings, lists, todo, quote,
    /// code, divider); the table/image kinds are declared here so the database CHECK and
    /// the wire contract stay complete while the UI grows into them.
    NoteBlockType {
        Paragraph => "paragraph",
        Heading1 => "heading_1",
        Heading2 => "heading_2",
        Heading3 => "heading_3",
        BulletedList => "bulleted_list",
        NumberedList => "numbered_list",
        Todo => "todo",
        Quote => "quote",
        Code => "code",
        Callout => "callout",
        Toggle => "toggle",
        Table => "table",
        Divider => "divider",
        Image => "image",
        PageLink => "page_link",
        Bookmark => "bookmark",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;

    #[test]
    fn db_literals_are_unique_within_each_enum() {
        macro_rules! check {
            ($t:ty) => {
                let set: HashSet<_> = <$t>::ALL.iter().map(|e| e.as_str()).collect();
                assert_eq!(set.len(), <$t>::ALL.len(), stringify!($t));
            };
        }
        macro_rules! all {
            ($($t:ty),* $(,)?) => { $( check!($t); )* };
        }
        all!(
            ReadingStatus,
            BookFormat,
            Condition,
            AuthorRole,
            BookSource,
            CoverSource,
            StorageMode,
            FileKind,
            ShelfKind,
            CustomColumnType,
            Direction,
            QuoteColor,
        );
    }

    /// Every variant must survive the trip through SQLite: `as_str` then `from_db`.
    ///
    /// Written per-enum rather than as one `.or_else` chain across unrelated types — each enum
    /// owns its own literals, so the meaningful property is that *each* one round-trips.
    #[test]
    fn round_trips_through_db_literals() {
        macro_rules! check {
            ($t:ty) => {
                for v in <$t>::ALL {
                    assert_eq!(
                        <$t>::from_db(v.as_str()),
                        Some(*v),
                        "{} failed to round-trip",
                        stringify!($t)
                    );
                }
            };
        }
        macro_rules! all {
            ($($t:ty),* $(,)?) => { $( check!($t); )* };
        }
        all!(
            ReadingStatus,
            BookFormat,
            Condition,
            AuthorRole,
            BookSource,
            CoverSource,
            StorageMode,
            FileKind,
            ShelfKind,
            CustomColumnType,
            Direction,
            QuoteColor,
        );
    }

    #[test]
    fn serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_string(&ReadingStatus::ToRead).unwrap(),
            r#""to_read""#
        );
        assert_eq!(
            serde_json::to_string(&Condition::LikeNew).unwrap(),
            r#""like_new""#
        );
    }

    #[test]
    fn unknown_db_values_are_forward_compatible() {
        assert!(ReadingStatus::from_db("on_hold_by_future_version").is_none());
    }
}
