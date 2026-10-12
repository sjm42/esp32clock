//! Display-only ISO-8859-15 (Latin-9) encoding, without unrelated encoding tables.

use std::borrow::Cow;

/// Preserve the previous encoder's ASCII fast path and decimal character-reference fallback.
pub(crate) fn encode(text: &str) -> Cow<'_, [u8]> {
    if text.is_ascii() {
        return Cow::Borrowed(text.as_bytes());
    }

    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let byte = match ch {
            '€' => Some(0xa4),
            'Š' => Some(0xa6),
            'š' => Some(0xa8),
            'Ž' => Some(0xb4),
            'ž' => Some(0xb8),
            'Œ' => Some(0xbc),
            'œ' => Some(0xbd),
            'Ÿ' => Some(0xbe),
            // These Latin-1 characters were replaced in Latin-9.
            '¤' | '¦' | '¨' | '´' | '¸' | '¼' | '½' | '¾' => None,
            '\u{0}'..='\u{ff}' => Some(ch as u8),
            _ => None,
        };

        if let Some(byte) = byte {
            bytes.push(byte);
        } else {
            append_numeric_reference(&mut bytes, ch);
        }
    }
    Cow::Owned(bytes)
}

fn append_numeric_reference(bytes: &mut Vec<u8>, ch: char) {
    // A Unicode scalar value has at most seven decimal digits.
    let mut digits = [0; 7];
    let mut start = digits.len();
    let mut value = ch as u32;
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    bytes.extend_from_slice(b"&#");
    bytes.extend_from_slice(&digits[start..]);
    bytes.push(b';');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrows_ascii_without_allocating() {
        for text in ["", "Clock 12:34", "\0\n\t\u{7f}"] {
            assert!(matches!(encode(text), Cow::Borrowed(_)));
            assert_eq!(encode(text).as_ref(), text.as_bytes());
        }
    }

    #[test]
    fn encodes_finnish_and_latin9_replacements() {
        assert_eq!(
            encode("ÄäÖöÅå €ŠšŽžŒœŸ").as_ref(),
            b"\xc4\xe4\xd6\xf6\xc5\xe5 \xa4\xa6\xa8\xb4\xb8\xbc\xbd\xbe"
        );
    }

    #[test]
    fn replaces_displaced_latin1_characters_with_decimal_references() {
        assert_eq!(
            encode("¤¦¨´¸¼½¾").as_ref(),
            b"&#164;&#166;&#168;&#180;&#184;&#188;&#189;&#190;"
        );
    }

    #[test]
    fn replaces_unmappable_unicode_with_decimal_references() {
        assert_eq!(encode("A中🙂\u{10ffff}Z").as_ref(), b"A&#20013;&#128578;&#1114111;Z");
    }

    #[test]
    fn preserves_all_other_latin1_bytes() {
        for byte in 0u8..=255 {
            if [0xa4, 0xa6, 0xa8, 0xb4, 0xb8, 0xbc, 0xbd, 0xbe].contains(&byte) {
                continue;
            }
            let mut utf8 = [0; 4];
            let text = char::from(byte).encode_utf8(&mut utf8);
            assert_eq!(encode(text).as_ref(), &[byte]);
        }
    }
}
