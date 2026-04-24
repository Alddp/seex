use serde::Serialize;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportedSymbol {
    pub lcsc_part: String,
    pub symbol_name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportedSymbolsResponse {
    pub scanned_path: String,
    pub items: Vec<ImportedSymbol>,
}

pub fn load_imported_symbols(output_path: &Path) -> Result<ImportedSymbolsResponse, String> {
    let scanned_path = output_path.display().to_string();
    let library_files = symbol_library_files(output_path)?;

    if library_files.is_empty() {
        return Ok(ImportedSymbolsResponse {
            scanned_path,
            items: Vec::new(),
        });
    }

    let mut items = BTreeSet::new();

    for path in library_files {
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read {}: {}", path.display(), err))?;
        let parsed = parse_kicad_symbol_lib(&content)
            .map_err(|err| format!("Failed to parse {}: {}", path.display(), err))?;
        items.extend(parsed);
    }

    Ok(ImportedSymbolsResponse {
        scanned_path,
        items: items.into_iter().collect(),
    })
}

pub fn unique_lcsc_parts(items: &[ImportedSymbol]) -> Vec<String> {
    let mut seen = HashSet::new();

    items
        .iter()
        .filter_map(|item| {
            if seen.insert(item.lcsc_part.clone()) {
                Some(item.lcsc_part.clone())
            } else {
                None
            }
        })
        .collect()
}

fn symbol_library_files(output_path: &Path) -> Result<Vec<PathBuf>, String> {
    if !output_path.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(output_path)
        .map_err(|err| format!("Failed to read {}: {}", output_path.display(), err))?;

    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("kicad_sym"))
        })
        .collect();

    files.sort();
    Ok(files)
}

fn parse_kicad_symbol_lib(content: &str) -> Result<Vec<ImportedSymbol>, String> {
    let mut items = Vec::new();

    for block in top_level_symbol_blocks(content)? {
        let symbol_name = head_string_after_keyword(block, "symbol")
            .ok_or_else(|| "symbol block is missing a name".to_string())?;
        let Some(lcsc_part) = property_value(block, "LCSC Part") else {
            continue;
        };

        items.push(ImportedSymbol {
            lcsc_part,
            symbol_name,
        });
    }

    Ok(items)
}

fn top_level_symbol_blocks(content: &str) -> Result<Vec<&str>, String> {
    let bytes = content.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0usize;
    let mut blocks = Vec::new();

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'(' => {
                if depth == 1 && starts_block_keyword(bytes, index, b"symbol") {
                    let end = matching_paren_end(bytes, index)
                        .ok_or_else(|| "unclosed top-level symbol block".to_string())?;
                    blocks.push(&content[index..end]);
                    index = end;
                    continue;
                }
                depth += 1;
            }
            b')' => {
                if depth == 0 {
                    return Err("unexpected ')' while scanning symbol library".to_string());
                }
                depth -= 1;
            }
            _ => {}
        }

        index += 1;
    }

    if in_string {
        return Err("unterminated string literal in symbol library".to_string());
    }

    if depth != 0 {
        return Err("unbalanced parentheses in symbol library".to_string());
    }

    Ok(blocks)
}

fn matching_paren_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut index = start;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }

        index += 1;
    }

    None
}

fn head_string_after_keyword(block: &str, keyword: &str) -> Option<String> {
    let prefix = format!("({keyword}");
    let rest = block.strip_prefix(&prefix)?;
    let start = rest.find('"')?;
    let end = quoted_string_end(rest, start)?;
    Some(unescape_kicad_string(&rest[start + 1..end]))
}

fn property_value(block: &str, property_name: &str) -> Option<String> {
    let bytes = block.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if starts_block_keyword(bytes, index, b"property") {
            let end = matching_paren_end(bytes, index)?;
            let property_block = &block[index..end];
            let strings = quoted_strings(property_block, 2);
            if strings.len() >= 2 && strings[0] == property_name {
                return Some(strings[1].clone());
            }
            index = end;
            continue;
        }
        index += 1;
    }

    None
}

fn starts_block_keyword(bytes: &[u8], index: usize, keyword: &[u8]) -> bool {
    let Some(rest) = bytes.get(index..) else {
        return false;
    };

    if rest.first() != Some(&b'(') {
        return false;
    }

    let Some(after_keyword) = rest.get(1 + keyword.len()) else {
        return false;
    };

    rest.get(1..1 + keyword.len()) == Some(keyword)
        && matches!(after_keyword, b' ' | b'\n' | b'\r' | b'\t')
}

fn quoted_strings(input: &str, limit: usize) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut results = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() && results.len() < limit {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        if let Some(end) = quoted_string_end(input, index) {
            results.push(unescape_kicad_string(&input[index + 1..end]));
            index = end + 1;
        } else {
            break;
        }
    }

    results
}

fn quoted_string_end(input: &str, start_quote: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = start_quote + 1;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return Some(index);
        }
        index += 1;
    }

    None
}

fn unescape_kicad_string(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                output.push(next);
            }
        } else {
            output.push(ch);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{ImportedSymbol, load_imported_symbols, parse_kicad_symbol_lib, unique_lcsc_parts};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "seex_imported_symbols_tests_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    fn symbol_block(symbol_name: &str, lcsc_part: Option<&str>) -> String {
        let lcsc_property = lcsc_part
            .map(|part| {
                format!(
                    "    (property\n      \"LCSC Part\"\n      \"{}\"\n      (id 5)\n      (at 0 0 0)\n      (effects (font (size 1.27 1.27) ) hide)\n    )\n",
                    part
                )
            })
            .unwrap_or_default();

        format!(
            "  (symbol \"{}\"\n    (property\n      \"Reference\"\n      \"U\"\n      (id 0)\n      (at 0 0 0)\n      (effects (font (size 1.27 1.27) ) )\n    )\n{}    (symbol \"{}_0_1\"\n    )\n  )\n",
            symbol_name, lcsc_property, symbol_name
        )
    }

    fn wrap_library(blocks: &[String]) -> String {
        format!(
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator seex-test)\n{}\n)\n",
            blocks.join("")
        )
    }

    #[test]
    fn parses_minimal_symbol_library() {
        let content = wrap_library(&[symbol_block("Device_C123", Some("C123"))]);

        let parsed = parse_kicad_symbol_lib(&content).expect("parse should succeed");

        assert_eq!(
            parsed,
            vec![ImportedSymbol {
                lcsc_part: "C123".to_string(),
                symbol_name: "Device_C123".to_string(),
            }]
        );
    }

    #[test]
    fn ignores_symbols_without_lcsc_part() {
        let content = wrap_library(&[
            symbol_block("Device_C123", Some("C123")),
            symbol_block("Graphic_Only", None),
        ]);

        let parsed = parse_kicad_symbol_lib(&content).expect("parse should succeed");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].symbol_name, "Device_C123");
    }

    #[test]
    fn load_imported_symbols_merges_multiple_files_and_deduplicates() {
        let root = test_root("multi_file");
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("alpha.kicad_sym"),
            wrap_library(&[
                symbol_block("Device_C123", Some("C123")),
                symbol_block("Amplifier_C456", Some("C456")),
            ]),
        )
        .unwrap();

        fs::write(
            root.join("beta.kicad_sym"),
            wrap_library(&[
                symbol_block("Device_C123", Some("C123")),
                symbol_block("Switch_C789", Some("C789")),
            ]),
        )
        .unwrap();

        let response = load_imported_symbols(&root).expect("scan should succeed");

        assert_eq!(response.scanned_path, root.display().to_string());
        assert_eq!(
            response.items,
            vec![
                ImportedSymbol {
                    lcsc_part: "C123".to_string(),
                    symbol_name: "Device_C123".to_string(),
                },
                ImportedSymbol {
                    lcsc_part: "C456".to_string(),
                    symbol_name: "Amplifier_C456".to_string(),
                },
                ImportedSymbol {
                    lcsc_part: "C789".to_string(),
                    symbol_name: "Switch_C789".to_string(),
                },
            ]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_directory_returns_empty_response() {
        let root = test_root("missing");

        let response = load_imported_symbols(&root).expect("scan should succeed");

        assert_eq!(response.scanned_path, root.display().to_string());
        assert!(response.items.is_empty());
    }

    #[test]
    fn unique_lcsc_parts_preserves_first_sorted_occurrence() {
        let parts = unique_lcsc_parts(&[
            ImportedSymbol {
                lcsc_part: "C123".to_string(),
                symbol_name: "Alpha".to_string(),
            },
            ImportedSymbol {
                lcsc_part: "C123".to_string(),
                symbol_name: "Beta".to_string(),
            },
            ImportedSymbol {
                lcsc_part: "C456".to_string(),
                symbol_name: "Gamma".to_string(),
            },
        ]);

        assert_eq!(parts, vec!["C123".to_string(), "C456".to_string()]);
    }
}
