use regex::Regex;

/// Extract a component ID from `content` using `keyword`.
///
/// Supports `||` separator to try multiple patterns in order (first match wins).
/// Each pattern can be `regex:...` or a literal substring.
pub fn extract_by_keyword(content: &str, keyword: &str) -> Option<String> {
    // Multi-pattern: try each separated by ||
    if keyword.contains("||") {
        for part in keyword.split("||") {
            let part = part.trim();
            if !part.is_empty() {
                if let Some(result) = extract_single(content, part) {
                    return Some(result);
                }
            }
        }
        return None;
    }
    extract_single(content, keyword)
}

fn extract_single(content: &str, keyword: &str) -> Option<String> {
    // --- regex mode ---
    if let Some(pattern) = keyword.strip_prefix("regex:") {
        let re = Regex::new(pattern).ok()?;
        let caps = re.captures(content)?;
        let m = caps.get(1).or_else(|| caps.get(0))?;
        let s = m.as_str().to_owned();
        if !s.is_empty() {
            return Some(s);
        }
        return None;
    }

    // --- literal mode ---
    if let Some(pos) = content.find(keyword) {
        let after_keyword = &content[pos + keyword.len()..];
        let extracted = after_keyword.trim();
        let result = extracted.split_whitespace().next().unwrap_or(extracted);
        let cleaned: String = result
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_info_block() {
        let content = "\u{540d}\u{79f0}\u{ff1a}6*6*9.5\u{63d2}\u{4ef6}\n\u{578b}\u{53f7}\u{ff1a}ZX-QC66-9.5CJ\n\u{54c1}\u{724c}\u{ff1a}Megastar\n\u{5c01}\u{88c5}\u{ff1a}\u{63d2}\u{4ef6}-4P\n\u{7f16}\u{53f7}\u{ff1a}C7470135";
        let kw = r"regex:\u{7f16}\u{53f7}[\u{ff1a}:]\s*(C\d+)||regex:(?m)^(C\d{3,})$";
        assert_eq!(extract_by_keyword(content, kw), Some("C7470135".into()));
    }

    #[test]
    fn bare_ccode() {
        let kw = r"regex:\u{7f16}\u{53f7}[\u{ff1a}:]\s*(C\d+)||regex:(?m)^(C\d{3,})$";
        assert_eq!(extract_by_keyword("C2040", kw), Some("C2040".into()));
    }

    #[test]
    fn bare_ccode_long() {
        let kw = r"regex:\u{7f16}\u{53f7}[\u{ff1a}:]\s*(C\d+)||regex:(?m)^(C\d{3,})$";
        assert_eq!(extract_by_keyword("C7470135", kw), Some("C7470135".into()));
    }

    #[test]
    fn no_false_match_model() {
        let content = "\u{578b}\u{53f7}\u{ff1a}ZX-QC66-9.5CJ";
        let kw = r"regex:\u{7f16}\u{53f7}[\u{ff1a}:]\s*(C\d+)||regex:(?m)^(C\d{3,})$";
        assert_eq!(extract_by_keyword(content, kw), None);
    }

    #[test]
    fn multi_pattern_first_wins() {
        let kw = "regex:A(\\d+)||regex:B(\\d+)";
        assert_eq!(extract_by_keyword("A123 B456", kw), Some("123".into()));
    }

    #[test]
    fn literal_keyword() {
        assert_eq!(
            extract_by_keyword(
                "\u{7f16}\u{53f7}\u{ff1a}C12345 foo",
                "\u{7f16}\u{53f7}\u{ff1a}"
            ),
            Some("C12345".into())
        );
    }
}
