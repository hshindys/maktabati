import { describe, expect, it } from "vitest";

import { ar } from "./ar";
import { en } from "./en";
import { catalogFor, translatorFor, translatorFrom } from "./index";

describe("catalogues", () => {
  it("has no key in English that is missing from Arabic", () => {
    // `en` is typed `Record<MessageKey, string>`, so a *missing* English key is already a
    // compile error. This guards the other direction: an English key with no Arabic original.
    const extra = Object.keys(en).filter((k) => !(k in ar));
    expect(extra).toEqual([]);
  });

  it("has no untranslated Arabic value that is byte-identical to English", () => {
    // A handful of values are legitimately identical (e.g. the Latin-script accent names are
    // not, but "Ctrl+K" is). Allow-list them explicitly so adding a new copy-paste is caught.
    const allowedIdentical = new Set<string>(["shell.searchHint"]);
    const suspicious = Object.keys(ar).filter(
      (k) => ar[k as keyof typeof ar] === en[k as keyof typeof en] && !allowedIdentical.has(k),
    );
    expect(suspicious).toEqual([]);
  });

  it("uses dotted, namespaced keys so they can be generated from Rust error codes", () => {
    for (const key of Object.keys(ar)) {
      expect(key).toMatch(/^[a-z][a-zA-Z0-9]*(\.[a-zA-Z0-9_]+)+$/);
    }
  });

  it("exposes the same catalog for a locale", () => {
    expect(catalogFor("ar")).toBe(ar);
    expect(catalogFor("en")).toBe(en);
  });
});

describe("translator", () => {
  it("returns the Arabic string for Arabic", () => {
    expect(translatorFor("ar")("nav.library")).toBe("المكتبة");
  });

  it("returns the English string for English", () => {
    expect(translatorFor("en")("nav.library")).toBe("Library");
  });

  it("interpolates placeholders", () => {
    const catalog = { ...en, "test.count": "{n} books · {title}" } as never;
    const t = translatorFrom(catalog);
    expect(t("test.count" as never, { n: 5, title: "Dune" })).toBe("5 books · Dune");
  });

  it("replaces every occurrence of a placeholder", () => {
    const catalog = { ...en, "test.dup": "{n} of {n}" } as never;
    const t = translatorFrom(catalog);
    expect(t("test.dup" as never, { n: 3 })).toBe("3 of 3");
  });

  it("leaves an unknown placeholder visible rather than blanking it", () => {
    const catalog = { ...en, "test.partial": "Hello {name}" } as never;
    const t = translatorFrom(catalog);
    expect(t("test.partial" as never, {})).toBe("Hello {name}");
  });
});

describe("error keys", () => {
  /**
   * Every code `DomainError::code` and `StorageError::code` can produce.
   *
   * `AppErrorDto.i18n_key` is derived as `error.{code}`, so a code with no entry here would
   * show a developer string to a user. Mirrored from `domain/src/error.rs` and
   * `storage/src/error.rs`; a new error code must be added here, in `ar.ts` and in `en.ts`.
   */
  const RUST_ERROR_CODES = [
    // crates/domain/src/error.rs
    "error.validation.isbn_invalid",
    "error.validation.empty_field",
    "error.validation.out_of_range",
    "error.validation.too_long",
    "error.validation.invalid_enum",
    "error.validation.not_found",
    "error.validation.duplicate",
    "error.validation.invalid_transition",
    "error.validation.revision_conflict",
    "error.security.unsafe_path",
    // crates/storage/src/error.rs
    "error.storage.not_found",
    "error.storage.duplicate",
    "error.storage.busy",
    "error.storage.migration",
    "error.storage.migration_checksum",
    "error.storage.serialization",
    "error.storage.io",
    "error.storage.sqlite",
    "error.storage.read_only",
    "error.storage.cannot_open",
    "error.storage.corrupt",
    "error.storage.unique_violation",
    "error.storage.foreign_key_violation",
    "error.storage.constraint_violation",
    // the catch-all for anything thrown without a domain/storage type
    "error.internal",
    "error.title",
  ];

  it("every error code raised by Rust has an Arabic message", () => {
    for (const code of RUST_ERROR_CODES) {
      expect(ar[code as keyof typeof ar], `missing in ar.ts: ${code}`).toBeTruthy();
    }
  });

  it("every error code raised by Rust has an English message", () => {
    for (const code of RUST_ERROR_CODES) {
      expect(en[code as keyof typeof en], `missing in en.ts: ${code}`).toBeTruthy();
    }
  });

  it("has no error.* key that is not a real Rust code", () => {
    // The other direction: a translated string nobody can ever reach is dead weight that
    // silently rots when the Rust code is renamed.
    const known = new Set(RUST_ERROR_CODES);
    const orphaned = Object.keys(ar).filter(
      (k) => (k.startsWith("error.") || k.startsWith("security.")) && !known.has(k),
    );
    expect(orphaned).toEqual([]);
  });
});
