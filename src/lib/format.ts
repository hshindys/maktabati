/**
 * Locale-aware formatting: digits, numbers, dates.
 *
 * Storage is always Western digits in a fixed Gregorian/epoch form (ADR-0009); every value is
 * rendered through this module at the edge, so switching the digit style or calendar is an
 * instant, pure display change with no migration.
 */

/** The two numeral systems the app can render. */
export type DigitStyle = "western" | "arabic-indic";

/** The calendars a date can be shown in. */
export type CalendarStyle = "gregorian" | "hijri" | "both";

/** U+0660 … U+0669 — the Arabic-Indic digits used across the Mashriq. */
const ARABIC_INDIC = ["٠", "١", "٢", "٣", "٤", "٥", "٦", "٧", "٨", "٩"] as const;

/**
 * Render the digits of `input` in the requested style.
 *
 * Non-digit characters (separators, letters, `-`, `.`) pass through untouched, so this is safe
 * to apply to already-formatted numbers, ISBNs and dates.
 */
export function toDigits(input: string | number, style: DigitStyle): string {
  const text = typeof input === "number" ? String(input) : input;
  if (style === "western") return text;
  return text.replace(/[0-9]/g, (d) => ARABIC_INDIC[Number(d)] ?? d);
}

/** Swap Arabic-Indic digits back to Western — used when parsing a field the user typed. */
export function toWesternDigits(input: string): string {
  return input.replace(/[٠-٩]/g, (d) => String(d.charCodeAt(0) - 0x0660));
}

/**
 * `true` when the string contains at least one Arabic-Indic digit, i.e. the user typed in the
 * other numeral system and we should normalise before validating.
 */
export function containsArabicIndicDigits(input: string): boolean {
  return /[٠-٩]/g.test(input);
}

/** Grouping separator per locale: `,` for English, `٬` (U+066C) for Arabic-Indic rendering. */
function groupSeparator(style: DigitStyle): string {
  return style === "arabic-indic" ? "٬" : ",";
}

/** Decimal separator per locale: `.` for English, `٫` (U+066B) for Arabic-Indic rendering. */
function decimalSeparator(style: DigitStyle): string {
  return style === "arabic-indic" ? "٫" : ".";
}

/** Options accepted by the formatting helpers. */
export interface FormatOptions {
  digits: DigitStyle;
  /** Force a fixed number of fraction digits (e.g. ratings shown as `4.5`). */
  minimumFractionDigits?: number;
  maximumFractionDigits?: number;
}

/** Format an integer with locale-appropriate grouping and digits. */
export function formatNumber(
  value: number,
  { digits, minimumFractionDigits = 0, maximumFractionDigits = 0 }: FormatOptions,
): string {
  const grouped = value.toLocaleString("en-US", {
    minimumFractionDigits,
    maximumFractionDigits,
  });
  // `en-US` gives `1,234.5`; swap the separators before transliterating the digits so the
  // Arabic-Indic form uses the Arabic separators rather than Latin ones.
  const withSeparators =
    digits === "arabic-indic"
      ? grouped.replace(/,/g, "␟").replace(/\./g, decimalSeparator(digits)).replace(/␟/g, groupSeparator(digits))
      : grouped;
  return toDigits(withSeparators, digits);
}

/** Shorthand for the common "big count" card: `1.2k`, `18.3k`. */
export function formatCompact(value: number, options: FormatOptions): string {
  const abs = Math.abs(value);
  if (abs < 1000) return formatNumber(value, options);
  const units: Array<[number, string]> = [
    [1e9, "B"],
    [1e6, "M"],
    [1e3, "k"],
  ];
  for (const [factor, suffix] of units) {
    if (abs >= factor) {
      const scaled = value / factor;
      const digits = Math.abs(scaled) < 10 ? 1 : 0;
      return (
        toDigits(
          scaled.toLocaleString("en-US", {
            minimumFractionDigits: digits,
            maximumFractionDigits: digits,
          }),
          options.digits,
        ) + suffix
      );
    }
  }
  return formatNumber(value, options);
}

/** Format a percentage that is already 0–100. */
export function formatPercent(value0to100: number, options: FormatOptions): string {
  return `${formatNumber(value0to100, { ...options, maximumFractionDigits: options.maximumFractionDigits ?? 0 })}%`;
}

/** A UTC timestamp in epoch milliseconds. */
export type EpochMs = number;

/**
 * Format a date.
 *
 * `locale` drives the *wording* (month names, ordering); `calendar` drives which calendar is
 * shown; `digits` drives the numeral system. All three are independent, so Arabic + Gregorian +
 * Arabic-Indic digits is a legitimate combination and is what the settings screen offers.
 */
export function formatDate(
  timestamp: EpochMs,
  locale: string,
  calendar: CalendarStyle,
  digits: DigitStyle,
): string {
  const date = new Date(timestamp);
  const opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "long", year: "numeric" };

  const gregorian = new Intl.DateTimeFormat(locale, { ...opts, calendar: "gregory" }).format(date);
  if (calendar === "gregorian") return toDigits(gregorian, digits);

  const hijri = formatHijri(date, locale);
  if (calendar === "hijri") return toDigits(hijri, digits);

  return `${toDigits(hijri, digits)} · ${toDigits(gregorian, digits)}`;
}

/**
 * Hijri date using the `islamic-umalqura` table — the calendar Saudi Arabia and most Arabic
 * publishing houses print on title pages.
 *
 * Falls back to the Gregorian calendar if the runtime's ICU build lacks the calendar, which
 * older WebKitGTK builds do; that is a degradation, not a crash.
 */
export function formatHijri(date: Date, locale: string): string {
  try {
    return new Intl.DateTimeFormat(`${locale}-u-ca-islamic-umalqura`, {
      day: "numeric",
      month: "long",
      year: "numeric",
    }).format(date);
  } catch {
    return new Intl.DateTimeFormat(locale, {
      day: "numeric",
      month: "long",
      year: "numeric",
      calendar: "gregory",
    }).format(date);
  }
}

/** The `YYYY-MM-DD` key used by `reading_sessions.session_date`. */
export function calendarDayKey(timestamp: EpochMs): string {
  return new Date(timestamp).toISOString().slice(0, 10);
}

/** Gregorian year of a timestamp, used by goals and the "finished this year" counter. */
export function gregorianYear(timestamp: EpochMs): number {
  return new Date(timestamp).getUTCFullYear();
}
