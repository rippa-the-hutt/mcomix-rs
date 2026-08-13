//! Natural (human) sorting of strings, mirroring MComix's `sort_paths_natural`.
//!
//! `page2.jpg` sorts before `page10.jpg`.

use std::cmp::Ordering;

/// Split a string into alternating non-digit / digit chunks.
fn split_chunks(s: &str) -> Vec<(bool, String)> {
    let mut chunks: Vec<(bool, String)> = Vec::new();
    let mut iter = s.chars().peekable();
    let mut current = String::new();
    let mut current_is_digit: Option<bool> = None;
    while let Some(c) = iter.next() {
        let is_digit = c.is_ascii_digit();
        match current_is_digit {
            None => {
                current.push(c);
                current_is_digit = Some(is_digit);
            }
            Some(d) if d == is_digit => current.push(c),
            Some(_) => {
                chunks.push((current_is_digit.unwrap(), std::mem::take(&mut current)));
                current.push(c);
                current_is_digit = Some(is_digit);
            }
        }
    }
    if !current.is_empty() {
        chunks.push((current_is_digit.unwrap_or(false), current));
    }
    chunks
}

/// Compare two digit strings numerically, ignoring leading zeros (like Python's
/// `int()` comparison, falling back to lexicographic order for equal values so
/// sorting stays stable and deterministic).
fn cmp_digit_chunks(a: &str, b: &str) -> Ordering {
    let a_trim = a.trim_start_matches('0');
    let b_trim = b.trim_start_matches('0');
    let (a_trim, b_trim) = if a_trim.is_empty() && b_trim.is_empty() {
        ("0", "0")
    } else {
        (if a_trim.is_empty() { "0" } else { a_trim }, if b_trim.is_empty() { "0" } else { b_trim })
    };
    // Compare by length first (more digits = larger number).
    match a_trim.len().cmp(&b_trim.len()) {
        Ordering::Equal => {}
        other => return other,
    }
    match a_trim.cmp(b_trim) {
        Ordering::Equal => a.len().cmp(&b.len()), // prefer fewer leading zeros
        other => other,
    }
}

/// Natural ordering comparator. `a < b` means `a` sorts before `b`.
/// Case-insensitive and digit-aware, mirroring `tools.alphanumeric_compare`
/// (which lowercases and compares digit groups numerically, digits before
/// letters).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let ac = split_chunks(&a.to_lowercase());
    let bc = split_chunks(&b.to_lowercase());
    let n = ac.len().min(bc.len());
    for i in 0..n {
        let (a_is_digit, a_text) = &ac[i];
        let (b_is_digit, b_text) = &bc[i];
        if a_is_digit != b_is_digit {
            // Digits sort before letters (Python's (0, int) < (1, str)).
            return b_is_digit.cmp(a_is_digit);
        }
        let ord = if *a_is_digit {
            cmp_digit_chunks(a_text, b_text)
        } else {
            a_text.cmp(b_text)
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    ac.len().cmp(&bc.len())
}

/// Sort a slice of strings in natural order.
pub fn natural_sort(items: &mut [String]) {
    items.sort_by(|a, b| natural_cmp(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_naturally() {
        let mut v = vec![
            "page10.jpg".to_string(),
            "page2.jpg".to_string(),
            "page1.jpg".to_string(),
            "page20.jpg".to_string(),
        ];
        natural_sort(&mut v);
        assert_eq!(
            v,
            vec![
                "page1.jpg".to_string(),
                "page2.jpg".to_string(),
                "page10.jpg".to_string(),
                "page20.jpg".to_string(),
            ]
        );
    }

    #[test]
    fn digits_first() {
        assert!(natural_cmp("2", "a") == Ordering::Less);
        assert!(natural_cmp("a", "b") == Ordering::Less);
        assert!(natural_cmp("a2", "a10") == Ordering::Less);
    }
}
