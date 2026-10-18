# ADR-0008 — Metadata providers: Open Library + Google Books, rate-limited and cached, with an adapter seam

**Status:** Accepted · **Date:** 2026-10-18

## Context

ISBN/title lookup must pre-fill the add-book wizard; covers must be fetchable by ISBN/title.
Research (`00-research.md` §1.4) established hard, documented limits and — critically — that
**no sanctioned, free, high-coverage Arabic metadata API exists**.

Verified limits:

| Source | Limit | Source of truth |
|---|---|---|
| Open Library | **1 req/s anonymous; 3 req/s identified** (`User-Agent: App (email)`) + explicit Do/Do-Not list | openlibrary.org/developers/api (fetched 2026-10-18) |
| Google Books v1 | **~1,000 requests/day per project** (project quota); 429 on exceed | Google docs + support thread |
| Wikidata WDQS | 60 s query time per 60 s, ≤30 error queries/min, ~5 parallel/IP; **endpoint migration in 2026–2027** | mediawiki.org + wikidata.org |

Rejected sources: OCLC Arabic Discovery Catalog (partnership), BABO (subscription), ARUC (no
public API), Neelwafurat & Jamalon (**no official public API found**), Noor Book (unofficial
third-party scraper), RapidAPI/ISBNdb (paid).

## Decision

1. **Providers (v1):** `OpenLibrary` (primary) and `GoogleBooks` (secondary/fallback). `Wikidata`
   is a **disabled-by-default enrichment** adapter with a configurable endpoint.
2. **One trait, many adapters** — `crates/metadata`:

   ```rust
   pub trait MetadataProvider: Send + Sync {
       fn id(&self) -> ProviderId;                       // ol | gbooks | wikidata
       fn lookup_by_isbn(&self, isbn: &Isbn) -> impl Future<Output = Result<Candidate, MetadataError>>;
       fn search_by_title(&self, q: &TitleQuery) -> impl Future<Output = Result<Vec<Candidate>, MetadataError>>;
       fn cover_url(&self, c: &Candidate) -> Option<CoverRef>;
   }
   ```

   Adding a future sanctioned Arabic source is a new adapter + a merge-policy entry, no other change.

3. **Rate limiting:** one **token-bucket per provider** (OL: 1/s identified; GBooks: a daily
   budget far below 1000 with a safety margin, e.g. 300). Exceeding it returns
   `MetadataError::RateLimited { retry_after }` — surfaced as an inline hint, never a modal loop.
4. **Caching:** responses cached in SQLite (`metadata_cache` keyed by
   `provider + query_type + normalized_key`) with fetched_at, TTL (default 30 days for lookups,
   180 days for negative results), and ETag/Last-Modified revalidation where supported.
   **Cache-first:** a cached answer never touches the network.
5. **Merge policy:** candidates are *proposals*, never auto-saved. The wizard shows the source and
   which fields came from where; the user confirms. Field precedence on multi-source: explicit
   user data > Google Books > Open Library > Wikidata, with `Option` semantics (never overwrite a
   non-empty local value with an empty remote one).
6. **Offline behavior:** `MetadataError::Offline` if the network is unreachable; the wizard simply
   stays manual. Nothing else in the app depends on this crate succeeding.
7. **Covers:** `GET` by ISBN from OL covers, then Google Books; downloaded bytes are hashed,
   stored under `covers/`, and multi-size thumbnails are generated locally (`image` +
   `fast_image_resize`). Never hotlinked at render time.
8. **Identified User-Agent** is configurable in Settings and sent on every request, per OL's
   documented requirement. No request fan-out across IPs; batch title lookups use `search.json`.

## Consequences

- ✅ Fully compliant with documented terms; resilient to a single provider being down/rate-limited.
- ✅ Offline-first preserved: cache + manual entry always work.
- ✅ Arabic books: ISBN-first, file-embedded metadata and CSV/manual entry remain the primary
  paths — a documented product reality, not a hidden gap.
- ⚠️ Google's 1,000/day is a hard ceiling shared by every user of our key. Mitigation: the daily
  budget is configurable, OL is tried first, and cache-first lookup means repeat lookups are free.
- ⚠️ Remote data quality is uneven (wrong covers, missing Arabic titles). Mitigation: user confirms
  before save; local value always wins on conflict.

## Alternatives considered

| Option | Why rejected |
|---|---|
| Scraping Neelwafurat/Jamalon/Goodreads | No sanctioned API; violates our own "no HTML scraping" rule and the spirit of OL's terms. |
| OCLC / BABO / ARUC | Subscription, partnership, or no machine interface. |
| ISBNdb / paid APIs | Cost + account; conflicts with a free local-first product. |
| Hardcover GraphQL API | Requires a personal token; catalog is Western-weighted. Kept as a *possible future* adapter, not v1. |
| Auto-save remote metadata | Would silently corrupt user data with bad remote records. |
