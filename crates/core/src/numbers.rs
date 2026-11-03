//! Spoken forms of numbers, money, percentages, ordinals, years and verses.
use crate::Word;
use regex::Regex;
use std::sync::OnceLock;

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [&str; 4] = ["hundred", "thousand", "million", "billion"];
const ORDINALS: [&str; 32] = [
    "zeroth",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
    "hundredth",
    "thousandth",
    "millionth",
    "billionth",
];
const YEAR_CONTEXT: &[&str] = &[
    "in",
    "since",
    "by",
    "of",
    "from",
    "until",
    "till",
    "before",
    "after",
    "during",
    "year",
    "circa",
    "around",
    "summer",
    "spring",
    "autumn",
    "fall",
    "winter",
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "sept",
    "oct",
    "nov",
    "dec",
];

/// Every number word `is_number_word` accepts.
pub(crate) fn number_words() -> impl Iterator<Item = &'static str> {
    ONES.into_iter()
        .chain(TENS)
        .chain(SCALES)
        .chain(ORDINALS)
        .filter(|w| !w.is_empty())
}

pub fn is_number_word(w: &str) -> bool {
    !w.is_empty()
        && (ONES.contains(&w) || TENS.contains(&w) || SCALES.contains(&w) || ORDINALS.contains(&w))
}

/// Words for `n` (no hyphens, no "and").
pub(crate) fn cardinal(n: u64) -> Vec<&'static str> {
    if n == 0 {
        return vec![ONES[0]];
    }
    let mut out = vec![];
    let mut rest = n;
    for (scale, word) in [
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ] {
        if rest >= scale {
            out.extend(cardinal(rest / scale));
            out.push(word);
            rest %= scale;
        }
    }
    if rest >= 100 {
        out.extend([ONES[(rest / 100) as usize], "hundred"]);
        rest %= 100;
    }
    if rest >= 20 {
        out.push(TENS[(rest / 10) as usize]);
        rest %= 10;
        if rest > 0 {
            out.push(ONES[rest as usize]);
        }
    } else if rest > 0 {
        out.push(ONES[rest as usize]);
    }
    out
}
pub(crate) fn ordinal(n: u64) -> Vec<String> {
    let mut out: Vec<String> = cardinal(n).into_iter().map(str::to_owned).collect();
    if let Some(last) = out.last_mut() {
        *last = match last.as_str() {
            "one" => "first".into(),
            "two" => "second".into(),
            "three" => "third".into(),
            "five" => "fifth".into(),
            "eight" => "eighth".into(),
            "nine" => "ninth".into(),
            "twelve" => "twelfth".into(),
            w if w.ends_with("ty") => format!("{}tieth", &w[..w.len() - 2]),
            w => format!("{w}th"),
        };
    }
    out
}
pub(crate) fn year(n: u64) -> Option<Vec<&'static str>> {
    if !(1100..=1999).contains(&n) && !(2010..=2099).contains(&n) {
        return None;
    }
    let (hi, lo) = (n / 100, n % 100);
    let mut out = cardinal(hi);
    match lo {
        0 => out.push("hundred"),
        1..=9 => out.extend(["oh", ONES[lo as usize]]),
        _ => out.extend(cardinal(lo)),
    }
    Some(out)
}

fn digits(d: &str) -> Vec<&'static str> {
    d.bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| ONES[usize::from(b - b'0')])
        .collect()
}
/// Cardinal for an ASCII digit run, digit by digit when longer than 12 or unparsable.
fn spell(d: &str) -> Vec<&'static str> {
    match d.parse::<u64>() {
        Ok(n) if d.len() <= 12 => cardinal(n),
        _ => digits(d),
    }
}
fn decimal(d: &str) -> Vec<&'static str> {
    match d.split_once('.') {
        Some((int, frac)) => {
            let mut out = spell(int);
            out.push("point");
            out.extend(digits(frac));
            out
        }
        None => spell(d),
    }
}
fn owned(words: Vec<&'static str>) -> Vec<String> {
    words.into_iter().map(str::to_owned).collect()
}

/// End (relative to `rest`) of a form match ending at `end`, if the form applies:
/// the next char is not alphanumeric, or `'s`/`’s` follows (and is included).
fn applies(rest: &str, end: usize) -> Option<usize> {
    let after = &rest[end..];
    for s in ["'s", "’s"] {
        if let Some(tail) = after.strip_prefix(s) {
            if !tail.chars().next().is_some_and(char::is_alphanumeric) {
                return Some(end + s.len());
            }
        }
    }
    (!after.chars().next().is_some_and(char::is_alphanumeric)).then_some(end)
}
/// Emits one unit: all but the last word are zero-width at `start`.
fn unit(out: &mut Vec<Word>, words: Vec<String>, start: usize, end: usize) {
    let n = words.len();
    for (i, text) in words.into_iter().enumerate() {
        let end = if i + 1 == n { end } else { start };
        out.push(Word { text, start, end });
    }
}

/// Tries the numeric token forms at byte `s` of `text`; on success pushes the
/// words and returns the end of the consumed span.
pub(crate) fn numeric(text: &str, s: usize, out: &mut Vec<Word>) -> Option<usize> {
    static FORMS: OnceLock<[Regex; 9]> = OnceLock::new();
    let forms = FORMS.get_or_init(|| {
        [
            r"^[0-9]{1,3}:[0-9]{1,3}(?:[-–][0-9]{1,3}(?::[0-9]{1,3})?)?",
            r"^\$[0-9]{1,3}(?:,[0-9]{3})+(?:\.[0-9]{2})?",
            r"^\$[0-9]+(?:\.[0-9]{2})?",
            r"^\$(?:[0-9]{1,3}(?:,[0-9]{3})+|[0-9]+)\.[0-9]+",
            r"^[0-9]+(?:\.[0-9]+)?%",
            r"^[0-9]{1,3}(?:,[0-9]{3})+",
            r"^[0-9]+\.[0-9]+",
            r"^[0-9]+(?i-u:st|nd|rd|th)",
            r"^[0-9]+",
        ]
        .map(|p| Regex::new(p).unwrap())
    });
    let rest = &text[s..];
    let (form, m, end) = forms.iter().enumerate().find_map(|(i, re)| {
        let m = re.find(rest)?;
        // "$2.5", "$1,000.5": not dollars and cents; the longer decimal
        // money form takes them.
        if matches!(i, 1 | 2) && forms[3].find(rest).is_some_and(|d| d.end() > m.end()) {
            return None;
        }
        Some((i, m.as_str(), applies(rest, m.end())?))
    })?;
    match form {
        0 => {
            let mut at = 0;
            let parts: Vec<_> = m.split(|c: char| !c.is_ascii_digit()).collect();
            for (i, p) in parts.iter().enumerate() {
                let start = at + m[at..].find(p).unwrap_or(0);
                at = start + p.len();
                let last = i + 1 == parts.len();
                let words = if p.len() >= 2 && p.bytes().all(|b| b == b'0') {
                    vec![]
                } else if p.len() == 2 && p.starts_with('0') {
                    vec!["oh", ONES[usize::from(p.as_bytes()[1] - b'0')]]
                } else if p.len() > 1 && p.starts_with('0') {
                    digits(p)
                } else {
                    spell(p)
                };
                let unit_end = if last { end } else { at };
                unit(out, owned(words), s + start, s + unit_end);
            }
        }
        1 | 2 => {
            let (d, c) = m[1..].split_once('.').unwrap_or((&m[1..], "00"));
            let d = d.replace(',', "");
            let (dollars, cents) = (d.parse::<u64>().ok(), c.parse::<u64>().unwrap_or(0));
            let mut words = vec![];
            if dollars != Some(0) || cents == 0 {
                words.extend(spell(&d));
                words.push(if dollars == Some(1) {
                    "dollar"
                } else {
                    "dollars"
                });
            }
            if cents > 0 {
                words.extend(cardinal(cents));
                words.push(if cents == 1 { "cent" } else { "cents" });
            }
            unit(out, owned(words), s, s + end);
        }
        3 => {
            let mut words = decimal(&m[1..].replace(',', ""));
            words.push("dollars");
            unit(out, owned(words), s, s + end);
        }
        4 => {
            let mut words = decimal(&m[..m.len() - 1]);
            words.push("percent");
            unit(out, owned(words), s, s + end);
        }
        5 => unit(out, owned(spell(&m.replace(',', ""))), s, s + end),
        6 => unit(out, owned(decimal(m)), s, s + end),
        7 => {
            // ASCII digits then a two-letter ASCII suffix.
            let d = m.trim_end_matches(|c: char| !c.is_ascii_digit());
            let words = match d.parse::<u64>() {
                Ok(n) if d.len() <= 12 => ordinal(n),
                _ => owned(digits(d)),
            };
            unit(out, words, s, s + end);
        }
        _ => {
            let prev = out.last().map(|w| w.text.as_str()).unwrap_or("");
            let context = YEAR_CONTEXT.contains(&prev);
            let words = if m.len() > 1 && m.starts_with('0') {
                digits(m)
            } else if let Some(y) = m
                .parse::<u64>()
                .ok()
                .filter(|_| m.len() == 4 && context)
                .and_then(year)
            {
                y
            } else {
                spell(m)
            };
            unit(out, owned(words), s, s + end);
        }
    }
    Some(s + end)
}
