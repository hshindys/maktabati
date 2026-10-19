//! A JSON value that can cross the IPC boundary (ADR-0002).
//!
//! Settings are stored as open-ended JSON (`key → value`), so *something* has to represent
//! arbitrary JSON on the command surface. That used to be a bare [`serde_json::Value`], which
//! cannot work: specta's built-in impl for `Value` is hand-written and expands structurally at
//! every use site, and because `Value` contains `Value` (`Array(Vec<Value>)`,
//! `Object(Map<String, Value>)`) that expansion never terminates. The exporter consumed
//! gigabytes and never returned.
//!
//! [`Json`] fixes it by being a *derived* named type instead. specta's registry inserts a
//! placeholder for a named datatype while its definition is being resolved, so a recursive
//! reference resolves back to that placeholder rather than re-entering — which is the machinery
//! the hand-written impl bypassed.
//!
//! The wire format is unchanged: `#[serde(untagged)]` serializes each variant as its bare
//! content, producing exactly the JSON that [`serde_json::Value`] produces.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A JSON number, exported to TypeScript as `number`.
///
/// `serde_json::Number` inlines as `{ f64 } | { i64 } | { u64 }`, and `specta-typescript`
/// refuses to emit `i64`/`u64` at all — `JSON.parse` truncates integers past 2⁵³ into a JS
/// `number`, so exporting them losslessly is impossible without BigInt. One number in a settings
/// value would therefore fail the *entire* export.
///
/// Controlling the definition is the fix: serde stays transparent, so the wire value is still a
/// real JSON number, and only the declared TypeScript type is narrowed to `number`.
///
/// That narrowing is honest rather than convenient. The webview has already run `JSON.parse` by
/// the time it reads this, so `number` describes what the frontend genuinely holds. The Rust side
/// keeps [`serde_json::Number`] unrounded, which is what matters for storage and comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonNumber(serde_json::Number);

impl JsonNumber {
    /// The underlying number, for the storage layer.
    #[must_use]
    pub fn into_inner(self) -> serde_json::Number {
        self.0
    }
}

impl From<serde_json::Number> for JsonNumber {
    fn from(number: serde_json::Number) -> Self {
        Self(number)
    }
}

impl From<JsonNumber> for serde_json::Number {
    fn from(number: JsonNumber) -> Self {
        number.0
    }
}

impl specta::Type for JsonNumber {
    fn definition(_: &mut specta::Types) -> specta::datatype::DataType {
        specta::datatype::DataType::Primitive(specta::datatype::Primitive::f64)
    }
}

/// JSON `null`, carried as a payload so the exporter emits a real `null`.
///
/// A *unit variant* (`enum Json { Null, … }`) is the obvious spelling, but under
/// `#[serde(untagged)]` specta skips its unit→empty-tuple transform (that path only runs for
/// variant-level `untagged`), so the variant keeps `Fields::Unit` — and a unit variant renders as
/// its name literal. The contract then claimed settings could be the string `"Null"` while the
/// wire value was `null`. A unit *struct* renders through `Fields::Unit` → `null`, which is what
/// serde serializes it as too, so both sides agree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct JsonNull;

/// Arbitrary JSON, mirroring [`serde_json::Value`] but safe to export to TypeScript.
///
/// Use this in any command signature that carries a dynamic payload — settings values are the
/// current example. Prefer a concrete type whenever the shape is actually known; this exists for
/// the cases where it deliberately is not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(untagged)]
pub enum Json {
    /// JSON `null`, carried as [`JsonNull`] so the exporter emits `null` rather than `"Null"`.
    Null(#[specta(inline)] JsonNull),
    /// JSON `true` / `false`.
    Bool(bool),
    /// A JSON number, through [`JsonNumber`] so the export does not trip over `i64`.
    Number(JsonNumber),
    /// A JSON string.
    String(String),
    /// A JSON array.
    Array(Vec<Json>),
    /// A JSON object.
    ///
    /// A [`BTreeMap`] keeps keys sorted, so serializing the same object twice always yields the
    /// same bytes and comparisons are stable.
    Object(BTreeMap<String, Json>),
}

impl Json {
    /// Convert back into [`serde_json::Value`], for code that talks to the storage layer.
    #[must_use]
    pub fn into_value(self) -> serde_json::Value {
        match self {
            Json::Null(_) => serde_json::Value::Null,
            Json::Bool(b) => serde_json::Value::Bool(b),
            Json::Number(n) => serde_json::Value::Number(n.into_inner()),
            Json::String(s) => serde_json::Value::String(s),
            Json::Array(a) => {
                serde_json::Value::Array(a.into_iter().map(Json::into_value).collect())
            }
            Json::Object(o) => {
                serde_json::Value::Object(o.into_iter().map(|(k, v)| (k, v.into_value())).collect())
            }
        }
    }
}

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Json::Null(JsonNull),
            serde_json::Value::Bool(b) => Json::Bool(b),
            serde_json::Value::Number(n) => Json::Number(n.into()),
            serde_json::Value::String(s) => Json::String(s),
            serde_json::Value::Array(a) => Json::Array(a.into_iter().map(Json::from).collect()),
            serde_json::Value::Object(o) => {
                Json::Object(o.into_iter().map(|(k, v)| (k, Json::from(v))).collect())
            }
        }
    }
}

impl From<Json> for serde_json::Value {
    fn from(value: Json) -> Self {
        value.into_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// The whole reason this type exists.
    ///
    /// Resolving `Json`'s definition must reach a registered placeholder instead of re-entering
    /// itself. If that ever regresses the real symptom is a hung, multi-gigabyte export inside
    /// `maktabati-app`'s bindings test — expensive to diagnose from there, cheap to catch here.
    #[test]
    fn resolving_the_definition_terminates() {
        let mut types = specta::Types::default();
        let definition = <Json as specta::Type>::definition(&mut types);
        let after_first = types.len();

        // A handful of registrations for a type that nests: `Json` plus the types it mentions.
        assert!(
            after_first > 0 && after_first <= 8,
            "expected a bounded set of registrations, got {after_first}"
        );

        // The invariant that actually matters. The impl this replaced grew on every resolution
        // and never returned, turning a type export into a multi-gigabyte hang; if resolution
        // ever becomes recursive again this is where it shows up, long before `maktabati-app`.
        for _ in 0..16 {
            let _resolved = <Json as specta::Type>::definition(&mut types);
        }
        assert_eq!(
            types.len(),
            after_first,
            "re-resolving must reach the same placeholder, not register more types"
        );

        // The definition itself must be stable too, or export order would decide the output.
        assert_eq!(
            definition,
            <Json as specta::Type>::definition(&mut types),
            "definition must not depend on how often it is asked"
        );
    }

    /// The conversion has to be lossless in both directions, or settings silently change shape
    /// when they pass through the command surface.
    #[test]
    fn round_trips_through_serde_json_value() {
        let cases = [
            "null",
            "true",
            "0",
            "-12.5",
            "9007199254740993",
            "\"theme\"",
            "[1, \"two\", null]",
            "{\"a\": {\"b\": [true, false]}, \"c\": null}",
        ];
        for case in cases {
            let value: serde_json::Value =
                serde_json::from_str(case).expect("fixture is valid JSON");
            let json = Json::from(value.clone());
            assert_eq!(serde_json::Value::from(json.clone()), value, "case: {case}");

            // And it must reach the wire byte-identically to `serde_json::Value`.
            let wire_direct = serde_json::to_string(&value).expect("value serializes");
            let wire_wrapped = serde_json::to_string(&json).expect("json serializes");
            assert_eq!(wire_wrapped, wire_direct, "wire drift for: {case}");
        }
    }

    /// Unpacked objects come back sorted, so equality does not depend on arrival order.
    #[test]
    fn object_keys_are_sorted() {
        let value: serde_json::Value =
            serde_json::from_str("{\"b\": 1, \"a\": 2}").expect("fixture is valid JSON");
        let wire = serde_json::to_string(&Json::from(value)).expect("json serializes");
        assert_eq!(wire, "{\"a\":2,\"b\":1}");
    }
}
