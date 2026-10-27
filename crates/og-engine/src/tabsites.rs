//! The text of the pages of web sites: Latin-1 pages, HTML entities,
//! percent-encoded addresses, and texts compared without case or accents.
//! The tablature sites themselves are read by [`crate::sites`].

/// The text of a page served as ISO-8859-1: every byte is one character.
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Replaces the HTML entities a page writes in its texts: the named ones the
/// sites use, and the numeric ones (`&#233;`, `&#x27;`).
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|&end| end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "eacute" => Some('é'),
            "egrave" => Some('è'),
            "ecirc" => Some('ê'),
            "agrave" => Some('à'),
            "acirc" => Some('â'),
            "ccedil" => Some('ç'),
            "ocirc" => Some('ô'),
            "ucirc" => Some('û'),
            "ugrave" => Some('ù'),
            "icirc" => Some('î'),
            "iuml" => Some('ï'),
            "euml" => Some('ë'),
            "oelig" => Some('œ'),
            _ => entity
                .strip_prefix('#')
                .and_then(|number| match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse().ok(),
                })
                .and_then(char::from_u32),
        };
        match replacement {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The text of a percent-encoded piece of URL.
pub fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%').then(|| text.get(i + 1..i + 3).and_then(|pair| u8::from_str_radix(pair, 16).ok())).flatten();
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A text as it is compared: lowercase, without accents.
pub fn fold(text: &str) -> String {
    text.chars()
        .map(|c| match c.to_lowercase().next().unwrap_or(c) {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            'œ' => 'o',
            'ß' => 's',
            other => other,
        })
        .collect()
}

/// Whether `name` holds every word of `query`, whatever the case and accents.
pub fn matches_words(name: &str, query: &str) -> bool {
    let name = fold(name);
    let mut words = fold(query);
    words.retain(|c| c.is_alphanumeric() || c.is_whitespace());
    let mut any = false;
    for word in words.split_whitespace() {
        any = true;
        if !name.contains(word) {
            return false;
        }
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin1_and_entities_give_the_text_of_a_french_page() {
        assert_eq!(latin1(&[0x54, 0xe9, 0x6c, 0xe9]), "Télé");
        assert_eq!(unescape("S&amp;M &eacute;t&eacute; &agrave; No&euml;l &#233;&#x27;&quot;&#39;&nbsp;x"), "S&M été à Noël é'\"' x");
        assert_eq!(unescape("a & b &unknown; &#99999999; & "), "a & b &unknown; &#99999999; & ", "what is not an entity stays");
        assert_eq!(unescape("fin &"), "fin &");
        assert_eq!(url_decode("Buckley%2CJeff+x%zz"), "Buckley,Jeff x%zz");
        assert_eq!(fold("Noël À"), "noel a");
        assert!(matches_words("Buckley Jeff", "jeff buckley") && matches_words("Noël Magne", "noel") && !matches_words("Nirvana", "nirvana unplugged") && !matches_words("Nirvana", " "));
    }
}
