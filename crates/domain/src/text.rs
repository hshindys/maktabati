//! Arabic-aware text normalization — the shared search/dedupe contract.
//!
//! Raw text is always stored as written. These functions produce the **normalized mirror**
//! (`title_norm`, `authors_norm`, `text_norm`, …) that FTS5 and duplicate detection use, and the
//! *same* function is applied to queries, so a query matches what was indexed.
//!
//! ## The pipeline (ADR-0004)
//!
//! 1. **NFD** decompose — this alone turns `أ/إ/آ` into `ا` + a combining mark.
//! 2. **Strip combining marks** (non-zero canonical combining class) → removes tashkeel
//!    (fatha/damma/kasra/sukun/shadda/tanwin) and the hamza marks left by step 1.
//! 3. **Strip tatweel** `U+0640` (ـ) and control/format characters.
//! 4. **Fold** the remaining letter variants that do not decompose:
//!    `ة→ه`, `ى→ي`, `ٱ→ا`, `ی→ي`, `ک→ك`, and Arabic-Indic / Extended Arabic-Indic digits → ASCII.
//! 5. **Keep only letters, digits and whitespace** (drops punctuation and symbols so
//!    `مقدمة…` and `مقدمة` produce the same key).
//! 6. **Collapse whitespace**, trim, **lowercase**.
//!
//! The function is **idempotent** — asserted by a property test — so it is safe to apply at write
//! time, at query time, and again during a reindex.
//!
//! ## Light stemming
//!
//! [`light_stem`] is a *conservative, Larkey-inspired* affix stripper. Research (see
//! `docs/00-research.md`) shows aggressive stemming can hurt precision, so stemming is
//! **opt-in** and never applied to the stored normalized text — only to the text handed to FTS5
//! while the setting is enabled, and the index can be rebuilt from source at any time.

use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

/// Unicode `Cf` (format) ranges that must never affect a match.
///
/// Rust exposes [`char::is_control`] for the `Cc` category but has no stable predicate for `Cf`,
/// so the ranges that can realistically reach a library record are listed explicitly rather than
/// pulling in a full general-category table for a single check.
///
/// Deliberately a *subset* of `Cf`: the requirement is only that an invisible character — a bidi
/// control, a zero-width joiner, a soft hyphen, a byte-order mark — can never make two strings
/// that a user sees as identical fail to match.
const FORMAT_RANGES: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),   // SOFT HYPHEN
    (0x0600, 0x0605),   // ARABIC NUMBER / CURRENCY SIGN, ARABIC NUMBER MARK ABOVE / BELOW
    (0x061C, 0x061C),   // ARABIC LETTER MARK
    (0x06DD, 0x06DD),   // ARABIC END OF AYAH
    (0x070F, 0x070F),   // SYRIAC ABBREVIATION MARK
    (0x180E, 0x180E),   // MONGOLIAN VOWEL SEPARATOR
    (0x200B, 0x200F),   // ZWSP · ZWNJ · ZWJ · LRM · RLM
    (0x202A, 0x202E),   // bidi embedding / override
    (0x2060, 0x2064),   // WORD JOINER · invisible operators
    (0x2066, 0x206F),   // bidi isolates · deprecated format controls
    (0xFEFF, 0xFEFF),   // BYTE ORDER MARK
    (0xFFF9, 0xFFFB),   // interlinear annotation
    (0x110BD, 0x110BD), // KAITHI NUMBER SIGN
    (0x1BCA0, 0x1BCA3), // shorthand format controls
    (0x1D173, 0x1D17A), // musical block begin / end
    (0xE0001, 0xE0001), // LANGUAGE TAG
    (0xE0020, 0xE007F), // TAG characters
];

/// `true` when `c` is an invisible format character that should be dropped before matching.
///
/// See [`FORMAT_RANGES`] for why these are listed rather than the whole `Cf` table.
#[must_use]
pub fn is_format_char(c: char) -> bool {
    let cp = c as u32;
    FORMAT_RANGES
        .iter()
        .any(|&(lo, hi)| (lo..=hi).contains(&cp))
}

/// Characters kept by [`normalize`]: letters, digits and whitespace.
#[inline]
fn is_kept(c: char) -> bool {
    c.is_alphanumeric() || c.is_whitespace()
}

/// Step 1–4: character-level folding without whitespace handling.
///
/// Exposed for tests and for callers that need folding without punctuation removal
/// (for example, display-friendly comparisons).
#[must_use]
pub fn fold_chars(input: &str) -> String {
    // NFD gives us alef-with-hamza/madda → bare alef + combining mark.
    let decomposed: String = input.nfd().collect();

    let mut out = String::with_capacity(decomposed.len());
    for c in decomposed.chars() {
        // Step 2: drop combining marks (tashkeel, hamza above/below, Latin accents…).
        if unicode_normalization::char::canonical_combining_class(c) != 0 {
            continue;
        }
        // Step 3: tatweel, Cc controls and invisible Cf format characters (ZWJ, bidi marks, BOM…).
        if c == '\u{0640}' || c.is_control() || is_format_char(c) {
            continue;
        }
        // Step 4: letter/digit folds that Unicode does not decompose for us.
        let folded = match c {
            '\u{0629}' => '\u{0647}',              // ة  ta marbuta  → ه
            '\u{0649}' | '\u{06CC}' => '\u{064A}', // ى / ی → ي
            '\u{0671}' => '\u{0627}',              // ٱ  alef wasla  → ا
            '\u{06A9}' => '\u{0643}',              // ک  keheh       → ك
            '\u{06D2}' => '\u{064A}',              // ے  yeh barree  → ي
            // Arabic-Indic digits ٠-٩ (U+0660..=0669)
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            // Extended Arabic-Indic digits ۰-۹ (U+06F0..=06F9) — Persian/Urdu keyboard
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            other => other,
        };
        out.push(folded);
    }
    out
}

/// Normalize text for storage in a `*_norm` column **and** for querying.
///
/// Idempotent: `normalize(normalize(s)) == normalize(s)`.
///
/// # Examples
///
/// ```
/// use maktabati_domain::text::normalize;
///
/// // Tashkeel, hamza variants and ta-marbuta do not block a match.
/// assert_eq!(normalize("الْقِرَاءَةُ"), normalize("القراءة"));
/// assert_eq!(normalize("إحسان"), normalize("احسان"));
/// assert_eq!(normalize("مكتبة"), normalize("مكتبه"));
///
/// // Arabic-Indic and Western digits normalize to the same key.
/// assert_eq!(normalize("١٩٧٨"), normalize("1978"));
///
/// // Punctuation is dropped, whitespace collapsed, Latin lowercased.
/// assert_eq!(normalize("  The Great Gatsby!! "), "the great gatsby");
/// ```
#[must_use]
pub fn normalize(input: &str) -> String {
    let folded = fold_chars(input);
    let mut out = String::with_capacity(folded.len());
    let mut last_was_space = true; // also trims leading whitespace

    for c in folded.chars() {
        if !is_kept(c) {
            // Not a letter/digit/space → drop it entirely (punctuation, symbols).
            continue;
        }
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            // Lowercase after folding so Turkish dotted-I style edge cases are handled last.
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            last_was_space = false;
        }
    }

    // Trim the trailing collapsed space, if any.
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Build the canonical duplicate-detection key for a title + primary author pair.
///
/// Two books are candidates for a merge when their ISBNs match **or** their dedupe keys match.
///
/// # Examples
///
/// ```
/// use maktabati_domain::text::dedupe_key;
/// assert_eq!(
///     dedupe_key("مقدمة في الحضارة الإسلامية!", "أحمد فكري"),
///     dedupe_key("مقدمة  في الحضارة الاسلامية", "احمد فكري")
/// );
/// ```
#[must_use]
pub fn dedupe_key(title: &str, primary_author: &str) -> String {
    let t = normalize(title);
    let a = normalize(primary_author);
    if a.is_empty() {
        t
    } else {
        format!("{t}\u{1}{a}")
    }
}

/// Conservative Arabic **light stemmer** (Larkey-inspired), used only when the user enables
/// light stemming in Settings.
///
/// Removes at most **one** prefix and **one** suffix, and never reduces a word below
/// [`MIN_STEM`] characters. Returns the input unchanged when nothing safe applies.
///
/// The affix lists come from the published Arabic light-stemming literature (Larkey et al.;
/// Kadri & Nie) collected in `docs/00-research.md` §6 — they are deliberately short, because
/// over-stemming groups semantically unrelated words and *lowers* precision.
///
/// # Examples
///
/// ```
/// use maktabati_domain::text::light_stem;
///
/// // Definite article removed…
/// assert_eq!(light_stem("الكتاب"), "كتاب");
/// // …and a plural suffix, without destroying short stems.
/// assert_eq!(light_stem("المكتبات"), "مكتب");
/// // Short words are left alone.
/// assert_eq!(light_stem("بيت"), "بيت");
/// ```
pub fn light_stem(word: &str) -> String {
    /// Never shrink below this many characters.
    const MIN_STEM: usize = 3;

    let chars: Vec<char> = normalize(word).chars().collect();
    if chars.len() < MIN_STEM + 1 {
        return chars.into_iter().collect();
    }

    // Longest-first so `وال` wins over `و`.
    const PREFIXES: &[&str] = &[
        "وال", "بال", "كال", "فال", "لال", "وال", "ال", "و", "ف", "ب", "ل", "ك", "س",
    ];
    const SUFFIXES: &[&str] = &[
        "تين", "تان", "ون", "ين", "ات", "ان", "وا", "ها", "هم", "هن", "كن", "نا", "كم", "تم", "ية",
        "يك", "ي", "ه", "ا", "ة", "ك", "ت", "ن", "و",
    ];

    let mut s: Vec<char> = chars;

    for p in PREFIXES {
        let p: Vec<char> = p.chars().collect();
        if s.len() > p.len() && s[..p.len()] == p[..] && (s.len() - p.len()) >= MIN_STEM {
            s = s[p.len()..].to_vec();
            break; // at most one prefix
        }
    }

    for suf in SUFFIXES {
        let suf: Vec<char> = suf.chars().collect();
        if s.len() > suf.len()
            && s[s.len() - suf.len()..] == suf[..]
            && (s.len() - suf.len()) >= MIN_STEM
        {
            s.truncate(s.len() - suf.len());
            break; // at most one suffix
        }
    }

    s.into_iter().collect()
}

/// Split normalized text into tokens and apply [`light_stem`] to each.
///
/// Used to build the query when light stemming is enabled, and to build the indexed text when
/// the setting is turned on (followed by `search::reindex_all()`).
#[must_use]
pub fn stem_tokens(normalized: &str) -> String {
    normalized
        .split(' ')
        .map(light_stem)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detect whether a string contains any Arabic-script letter.
#[must_use]
pub fn contains_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

/// Produce a stable, file-system-safe slug (used for generated placeholder names and exports).
#[must_use]
pub fn slug(input: &str) -> String {
    let n = normalize(input);
    if n.is_empty() {
        return "untitled".to_owned();
    }
    n.split(' ').take(8).collect::<Vec<_>>().join("-")
}

/// Distinct tokens of a normalized string — handy for tests and for tag comparisons.
#[must_use]
pub fn tokens(normalized: &str) -> BTreeSet<&str> {
    normalized.split(' ').filter(|t| !t.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    // ── format characters (the documented `Cf` subset) ────────────────────────

    /// Every range endpoint is detected.
    ///
    /// A typo in one bound of `FORMAT_RANGES` would otherwise let an invisible character through
    /// silently, and it would only ever show up as a book that "should have matched" not matching.
    #[test]
    fn format_char_covers_every_listed_range() {
        for &(lo, hi) in FORMAT_RANGES {
            for cp in [lo, hi, (lo + hi) / 2] {
                let c = char::from_u32(cp).expect("table entries must be scalar values");
                assert!(
                    is_format_char(c),
                    "U+{cp:04X} is listed in FORMAT_RANGES but is not detected"
                );
            }
        }
    }

    /// The table is sorted and disjoint, so the linear scan has no ambiguous entry.
    ///
    /// `is_format_char` is `any` over the table; an overlapping or reversed pair would make the
    /// result depend on declaration order rather than on the code point.
    #[test]
    fn format_ranges_are_sorted_and_disjoint() {
        for w in FORMAT_RANGES.windows(2) {
            assert!(
                w[0].1 < w[1].0,
                "FORMAT_RANGES is out of order or overlapping: U+{:04X}..=U+{:04X} then U+{:04X}..=U+{:04X}",
                w[0].0, w[0].1, w[1].0, w[1].1
            );
        }
    }

    /// Nothing a user can see is ever treated as format.
    ///
    /// This is the case that actually matters. Over-matching deletes real content from the search
    /// index, which is far worse than the under-matching it was meant to prevent — so ordinary
    /// Latin and Arabic letters, both digit forms, whitespace and punctuation all have to survive.
    #[test]
    fn format_char_never_matches_visible_text() {
        let visible = [
            'a', 'Z', '0', '-', ' ', '\t', // latin, digits, whitespace
            'م', 'ك', 'ة', // arabic letters, incl. ta-marbuta
            '٢', '٠', // arabic-indic digits
            '،', '؟', '.', '—', // arabic + latin punctuation
        ];
        for c in visible {
            assert!(
                !is_format_char(c),
                "U+{:04X} is visible text but was classified as format",
                c as u32
            );
        }
    }

    // ── fixtures: pairs that MUST match after normalization ────────────────────
    const MATCHING_PAIRS: &[(&str, &str)] = &[
        // tashkeel
        ("الْقِرَاءَةُ", "القراءة"),
        ("مُحَمَّد", "محمد"),
        ("بِسْمِ اللهِ", "بسم الله"),
        // alef / hamza variants
        ("إحسان", "احسان"),
        ("آمن", "امن"),
        ("أرض", "ارض"),
        ("إيّاك", "اياك"),
        // ya / alef maqsura
        ("مصطفى", "مصطفي"),
        ("هدى", "هدي"),
        // ta marbuta
        ("مكتبة", "مكتبه"),
        ("قراءة", "قراءه"),
        // tatweel
        ("كتــاب", "كتاب"),
        // digits
        ("١٩٧٨", "1978"),
        ("۲۰۲۴", "2024"),
        // punctuation + case + spacing
        ("  The   Great Gatsby!! ", "the great gatsby"),
        ("مقدمة في الحضارة!", "مقدمة في الحضارة"),
    ];

    #[test]
    fn matching_pairs_normalize_identically() {
        for (a, b) in MATCHING_PAIRS {
            assert_eq!(normalize(a), normalize(b), "pair: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn normalization_is_idempotent_on_fixtures() {
        for (a, b) in MATCHING_PAIRS {
            assert_eq!(normalize(&normalize(a)), normalize(a), "input: {a:?}");
            assert_eq!(normalize(&normalize(b)), normalize(b), "input: {b:?}");
        }
    }

    #[test]
    fn distinct_words_stay_distinct() {
        // Folding must not be so aggressive that unrelated words collide.
        assert_ne!(normalize("قراءة"), normalize("كراءة"));
        assert_ne!(normalize("كتاب"), normalize("كتب"));
        assert_ne!(normalize("علم"), normalize("عمل"));
    }

    #[test]
    fn mixed_arabic_latin_query() {
        assert_eq!(
            normalize("التاريخ Modern History"),
            "التاريخ modern history"
        );
    }

    #[test]
    fn digits_normalize_to_ascii() {
        // Ta-marbuta folds to heh everywhere, not just in the second word: the old expectation
        // kept `ة` in `الطبعة` while folding it in `الثالثة`.
        assert_eq!(normalize("الطبعة الثالثة ٢٠٢٤"), "الطبعه الثالثه 2024");
    }

    #[test]
    fn dedupe_key_ignores_author_script_differences() {
        assert_eq!(
            dedupe_key("الأيام", "طه حسين"),
            dedupe_key("الايام", "طه حسين")
        );
        // Different authors ⇒ different keys even for the same title.
        assert_ne!(
            dedupe_key("البؤساء", "فيكتور هوجو"),
            dedupe_key("البؤساء", "تولستوي")
        );
    }

    #[test]
    fn light_stem_removes_article_and_plural() {
        assert_eq!(light_stem("الكتاب"), "كتاب");
        assert_eq!(light_stem("المكتبات"), "مكتب");
        assert_eq!(light_stem("المكتبة"), "مكتب");
    }

    #[test]
    fn light_stem_never_destroys_short_words() {
        assert_eq!(light_stem("بيت"), "بيت");
        assert_eq!(light_stem("علم"), "علم");
        assert_eq!(light_stem("قلم"), "قلم");
    }

    #[test]
    fn light_stem_is_safe_on_arbitrary_text() {
        for s in ["", "ا", "ال", "وال", "!!", "hello world", "القرآن الكريم"] {
            let stemmed = light_stem(s);
            // The guard matches the proptest further down: what matters is that stemming never
            // empties a token `normalize` itself keeps. Punctuation-only input is dropped by the
            // documented punctuation step before stemming runs, so `"!!"` legitimately yields "".
            if !normalize(s).is_empty() {
                assert!(!stemmed.is_empty(), "stemming destroyed {s:?}");
            }
        }
    }

    #[test]
    fn contains_arabic_detects_script() {
        assert!(contains_arabic("كتاب"));
        assert!(!contains_arabic("book"));
        assert!(contains_arabic("كتاب book"));
    }

    #[test]
    fn slug_is_filesystem_safe() {
        let s = slug("مقدمة في الحضارة / الإسلامية: جزء أول!");
        assert!(!s.contains('/'));
        assert!(!s.contains(':'));
        assert!(!s.contains('!'));
    }

    // ── properties ────────────────────────────────────────────────────────────
    proptest! {
        /// The single most important invariant: normalizing twice equals normalizing once.
        /// (Write-time and query-time normalization must agree forever.)
        #[test]
        fn prop_normalize_is_idempotent(s in ".*") {
            let once = normalize(&s);
            prop_assert_eq!(normalize(&once), once);
        }

        /// Normalization never invents or destroys content: the output is a subsequence of the
        /// input modulo case, whitespace collapsing and the documented letter folds.
        /// (Weaker but still useful: output has no leading/trailing/double spaces and no
        /// punctuation.)
        #[test]
        fn prop_normalize_output_is_well_formed(s in ".*") {
            let n = normalize(&s);
            prop_assert!(!n.starts_with(' '));
            prop_assert!(!n.ends_with(' '));
            prop_assert!(!n.contains("  "));
            prop_assert!(n.chars().all(|c| c.is_alphanumeric() || c == ' '));
        }

        /// Empty/whitespace-only input normalizes to empty (never panics).
        #[test]
        fn prop_normalize_empty(s in "\\s*") {
            prop_assert!(normalize(&s).is_empty());
        }

        /// Light stemming never grows the word and never empties a non-empty word.
        #[test]
        fn prop_light_stem_is_non_expansive(s in "\\P{Cc}\\P{Cf}*") {
            let stemmed = light_stem(&s);
            prop_assert!(stemmed.chars().count() <= normalize(&s).chars().count());
            if !normalize(&s).is_empty() {
                prop_assert!(!stemmed.is_empty());
            }
        }

        /// Stemming a normalized string keeps it well-formed (idempotent-ish safety net:
        /// applying the whole pipeline twice never panics and never grows).
        #[test]
        fn prop_stem_tokens_safe(s in ".*") {
            let n = normalize(&s);
            let once = stem_tokens(&n);
            prop_assert!(once.chars().count() <= n.chars().count() + 1);
        }
    }

    /// The documented end-to-end pipeline, pinned as a snapshot: every step — diacritic
    /// stripping, ta-marbuta fold, digit fold, punctuation drop — in one assertion.
    #[test]
    fn normalize_matches_the_documented_pipeline() {
        insta::assert_snapshot!(
            normalize("مُقَدِّمَةٌ فِي التَّارِيخِ الْإِسْلَامِيّ — الطبعة الثانية (٢٠٢٤)"),
            @"مقدمه في التاريخ الاسلامي الطبعه الثانيه 2024"
        );
    }
}
