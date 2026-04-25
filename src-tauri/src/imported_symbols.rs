use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImportedSymbol {
    pub lcsc_part: String,
    pub symbol_name: String,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ImportedSymbolsResponse {
    pub scanned_path: String,
    pub items: Vec<ImportedSymbol>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImportedSymbolUpdateRequest {
    pub source_file: String,
    pub symbol_name: String,
    pub new_symbol_name: String,
    pub lcsc_part: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ImportedSymbolDeleteRequest {
    pub source_file: String,
    pub symbol_name: String,
    #[serde(default)]
    pub lcsc_part: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryFile {
    path: PathBuf,
    source_file: String,
}

#[derive(Debug, Clone)]
struct SymbolBlock<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    symbol_name: String,
    lcsc_part: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DeletedGeneratedAssets {
    footprints: usize,
    models: usize,
    checkpoint_entries: usize,
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

    for library_file in library_files {
        let content = fs::read_to_string(&library_file.path)
            .map_err(|err| format!("Failed to read {}: {}", library_file.path.display(), err))?;
        let parsed = parse_kicad_symbol_lib(&content, &library_file.source_file)
            .map_err(|err| format!("Failed to parse {}: {}", library_file.path.display(), err))?;
        items.extend(parsed);
    }

    Ok(ImportedSymbolsResponse {
        scanned_path,
        items: items.into_iter().collect(),
    })
}

pub fn update_imported_symbol(
    output_path: &Path,
    request: ImportedSymbolUpdateRequest,
) -> Result<String, String> {
    let new_symbol_name = request.new_symbol_name.trim();
    let new_lcsc_part = request.lcsc_part.trim();

    if new_symbol_name.is_empty() {
        return Err("Symbol name cannot be empty".to_string());
    }
    if new_lcsc_part.is_empty() {
        return Err("LCSC Part cannot be empty".to_string());
    }

    let library_path = ensure_library_is_within_output(output_path, &request.source_file)?;
    let content = fs::read_to_string(&library_path)
        .map_err(|err| format!("Failed to read {}: {}", library_path.display(), err))?;
    let blocks = top_level_symbol_blocks(&content)?;
    let current = blocks
        .iter()
        .find(|block| block.symbol_name == request.symbol_name)
        .ok_or_else(|| {
            format!(
                "Symbol {} was not found in {}",
                request.symbol_name, request.source_file
            )
        })?;

    ensure_symbol_name_available(&blocks, &request.symbol_name, new_symbol_name)?;
    let updated_block = update_symbol_block(
        current.text,
        &request.symbol_name,
        new_symbol_name,
        new_lcsc_part,
    )?;
    let updated_content =
        apply_replacements(&content, vec![(current.start..current.end, updated_block)])?;
    fs::write(&library_path, updated_content)
        .map_err(|err| format!("Failed to write {}: {}", library_path.display(), err))?;

    Ok(format!(
        "Updated {} in {}",
        request.symbol_name, request.source_file
    ))
}

pub fn delete_imported_symbol(
    output_path: &Path,
    request: ImportedSymbolDeleteRequest,
) -> Result<String, String> {
    let library_path = ensure_library_is_within_output(output_path, &request.source_file)?;
    let content = fs::read_to_string(&library_path)
        .map_err(|err| format!("Failed to read {}: {}", library_path.display(), err))?;
    let blocks = top_level_symbol_blocks(&content)?;
    let current = blocks
        .iter()
        .find(|block| block.symbol_name == request.symbol_name)
        .ok_or_else(|| {
            format!(
                "Symbol {} was not found in {}",
                request.symbol_name, request.source_file
            )
        })?;

    let lcsc_part = current.lcsc_part.clone().or_else(|| {
        request
            .lcsc_part
            .as_deref()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
    });
    let footprint_name = footprint_name_from_symbol_block(current.text)
        .unwrap_or_else(|| current.symbol_name.clone());

    let updated_content = delete_symbol_block(&content, current);
    fs::write(&library_path, updated_content)
        .map_err(|err| format!("Failed to write {}: {}", library_path.display(), err))?;

    let deleted_assets = delete_generated_assets(
        output_path,
        &library_path,
        &footprint_name,
        lcsc_part.as_deref(),
    )?;

    Ok(format_delete_result(
        &request.symbol_name,
        &request.source_file,
        &deleted_assets,
    ))
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

fn symbol_library_files(output_path: &Path) -> Result<Vec<LibraryFile>, String> {
    if !output_path.exists() {
        return Ok(Vec::new());
    }

    let output_root = fs::canonicalize(output_path)
        .map_err(|err| format!("Failed to access {}: {}", output_path.display(), err))?;
    let entries = fs::read_dir(output_path)
        .map_err(|err| format!("Failed to read {}: {}", output_path.display(), err))?;

    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|err| format!("Failed to read directory entry: {}", err))?;
        let path = entry.path();
        if !is_kicad_symbol_file(&path) {
            continue;
        }

        let source_file = entry
            .file_name()
            .into_string()
            .map_err(|_| format!("Library filename is not valid UTF-8: {}", path.display()))?;
        let canonical = fs::canonicalize(&path)
            .map_err(|err| format!("Failed to access {}: {}", path.display(), err))?;
        if !canonical.starts_with(&output_root) {
            return Err(format!(
                "Refusing to read library outside {}: {}",
                output_path.display(),
                path.display()
            ));
        }
        if !canonical.is_file() {
            return Err(format!(
                "Library path is not a file: {}",
                canonical.display()
            ));
        }

        files.push(LibraryFile {
            path: canonical,
            source_file,
        });
    }

    files.sort_by(|left, right| left.source_file.cmp(&right.source_file));
    Ok(files)
}

fn parse_kicad_symbol_lib(content: &str, source_file: &str) -> Result<Vec<ImportedSymbol>, String> {
    let mut items = Vec::new();

    for block in top_level_symbol_blocks(content)? {
        let Some(lcsc_part) = block.lcsc_part else {
            continue;
        };

        items.push(ImportedSymbol {
            lcsc_part,
            symbol_name: block.symbol_name,
            source_file: source_file.to_string(),
        });
    }

    Ok(items)
}

fn top_level_symbol_blocks(content: &str) -> Result<Vec<SymbolBlock<'_>>, String> {
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
                    let text = &content[index..end];
                    let symbol_name = head_string_after_keyword(text, "symbol")
                        .ok_or_else(|| "symbol block is missing a name".to_string())?;
                    let lcsc_part = property_value(text, "LCSC Part");
                    blocks.push(SymbolBlock {
                        text,
                        start: index,
                        end,
                        symbol_name,
                        lcsc_part,
                    });
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
    let range = string_after_keyword_range(block, 0, keyword)?;
    Some(unescape_kicad_string(&block[range]))
}

fn property_value(block: &str, property_name: &str) -> Option<String> {
    let range = property_value_range(block, property_name)?;
    Some(unescape_kicad_string(&block[range]))
}

fn footprint_name_from_symbol_block(block: &str) -> Option<String> {
    let footprint = property_value(block, "Footprint")?;
    let trimmed = footprint.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        trimmed
            .rsplit_once(':')
            .map(|(_, name)| name)
            .unwrap_or(trimmed)
            .to_string(),
    )
}

fn property_value_range(block: &str, property_name: &str) -> Option<Range<usize>> {
    let bytes = block.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if starts_block_keyword(bytes, index, b"property") {
            let end = matching_paren_end(bytes, index)?;
            let property_block = &block[index..end];
            let ranges = quoted_string_ranges(property_block, 2);
            if ranges.len() >= 2
                && unescape_kicad_string(&property_block[ranges[0].clone()]) == property_name
            {
                let value = ranges[1].clone();
                return Some(index + value.start..index + value.end);
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

fn quoted_string_ranges(input: &str, limit: usize) -> Vec<Range<usize>> {
    let bytes = input.as_bytes();
    let mut results = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() && results.len() < limit {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        if let Some(end) = quoted_string_end(input, index) {
            results.push(index + 1..end);
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

fn string_after_keyword_range(block: &str, start: usize, keyword: &str) -> Option<Range<usize>> {
    let prefix = format!("({keyword}");
    let rest = block.get(start..)?;
    if !rest.starts_with(&prefix) {
        return None;
    }

    let content_start = start + prefix.len();
    let quote_offset = block.get(content_start..)?.find('"')?;
    let quote_start = content_start + quote_offset;
    let quote_end = quoted_string_end(block, quote_start)?;
    Some(quote_start + 1..quote_end)
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

fn escape_kicad_string(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' | '"' => {
                output.push('\\');
                output.push(ch);
            }
            _ => output.push(ch),
        }
    }
    output
}

fn ensure_library_is_within_output(
    output_path: &Path,
    source_file: &str,
) -> Result<PathBuf, String> {
    let source_file = normalize_source_file(source_file)?;
    let output_root = fs::canonicalize(output_path)
        .map_err(|err| format!("Failed to access {}: {}", output_path.display(), err))?;
    let joined = output_path.join(&source_file);
    let library_path = fs::canonicalize(&joined)
        .map_err(|err| format!("Failed to access {}: {}", joined.display(), err))?;
    if !library_path.starts_with(&output_root) {
        return Err("Source library file must stay inside the nlbn output directory".to_string());
    }
    if !library_path.is_file() {
        return Err(format!(
            "Library path is not a file: {}",
            library_path.display()
        ));
    }

    Ok(library_path)
}

fn normalize_source_file(source_file: &str) -> Result<String, String> {
    if source_file.trim().is_empty() {
        return Err("Source library file is required".to_string());
    }

    let path = Path::new(source_file);
    let mut components = path.components();
    let Some(Component::Normal(component)) = components.next() else {
        return Err("Source library file must stay inside the nlbn output directory".to_string());
    };

    if components.next().is_some() {
        return Err("Source library file must stay inside the nlbn output directory".to_string());
    }

    let normalized = component
        .to_str()
        .ok_or_else(|| "Source library filename is not valid UTF-8".to_string())?
        .to_string();
    if !is_kicad_symbol_file(Path::new(&normalized)) {
        return Err("Source library file must be a .kicad_sym library".to_string());
    }

    Ok(normalized)
}

fn ensure_symbol_name_available(
    blocks: &[SymbolBlock<'_>],
    current_name: &str,
    new_name: &str,
) -> Result<(), String> {
    if current_name == new_name {
        return Ok(());
    }

    if blocks.iter().any(|block| block.symbol_name == new_name) {
        return Err(format!(
            "Symbol {} already exists in this library",
            new_name
        ));
    }

    Ok(())
}

fn update_symbol_block(
    block: &str,
    current_name: &str,
    new_name: &str,
    new_lcsc_part: &str,
) -> Result<String, String> {
    let mut replacements = rename_symbol_names_in_block(block, current_name, new_name)?;
    let lcsc_range = property_value_range(block, "LCSC Part")
        .ok_or_else(|| "Symbol is missing LCSC Part property".to_string())?;
    replacements.push((lcsc_range, escape_kicad_string(new_lcsc_part)));
    apply_replacements(block, replacements)
}

fn rename_symbol_names_in_block(
    block: &str,
    current_name: &str,
    new_name: &str,
) -> Result<Vec<(Range<usize>, String)>, String> {
    let bytes = block.as_bytes();
    let mut index = 0usize;
    let mut replacements = Vec::new();

    while index < bytes.len() {
        if starts_block_keyword(bytes, index, b"symbol") {
            let range = symbol_head_string_range(block, index)
                .ok_or_else(|| "symbol block is missing a name".to_string())?;
            let existing = unescape_kicad_string(&block[range.clone()]);
            if existing == current_name {
                replacements.push((range, escape_kicad_string(new_name)));
            } else if let Some(suffix) = existing.strip_prefix(current_name)
                && suffix.starts_with('_')
            {
                replacements.push((range, escape_kicad_string(&format!("{new_name}{suffix}"))));
            }
        }
        index += 1;
    }

    Ok(replacements)
}

fn symbol_head_string_range(block: &str, start: usize) -> Option<Range<usize>> {
    string_after_keyword_range(block, start, "symbol")
}

fn apply_replacements(
    input: &str,
    mut replacements: Vec<(Range<usize>, String)>,
) -> Result<String, String> {
    replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.0.start));
    let mut result = input.to_string();
    let mut previous_start = input.len();

    for (range, replacement) in replacements {
        if range.start > range.end || range.end > result.len() {
            return Err("replacement range is out of bounds".to_string());
        }
        if range.end > previous_start {
            return Err("replacement ranges overlap".to_string());
        }
        result.replace_range(range.clone(), &replacement);
        previous_start = range.start;
    }

    Ok(result)
}

fn delete_symbol_block(content: &str, block: &SymbolBlock<'_>) -> String {
    let mut start = line_start(content, block.start);
    if start == 0 {
        start = block.start;
    }

    let mut end = block.end;
    if let Some(rest) = content.get(end..) {
        if rest.starts_with("\r\n") {
            end += 2;
        } else if rest.starts_with('\n') {
            end += 1;
        }
    }

    let mut updated = String::with_capacity(content.len().saturating_sub(end - start));
    updated.push_str(&content[..start]);
    updated.push_str(&content[end..]);
    updated
}

fn delete_generated_assets(
    output_path: &Path,
    library_path: &Path,
    footprint_name: &str,
    lcsc_part: Option<&str>,
) -> Result<DeletedGeneratedAssets, String> {
    let mut deleted = DeletedGeneratedAssets::default();
    let Some(library_name) = library_path.file_stem().and_then(|name| name.to_str()) else {
        return Ok(deleted);
    };

    let pretty_dir = output_path.join(format!("{library_name}.pretty"));
    let footprint_path = pretty_dir.join(format!("{footprint_name}.kicad_mod"));
    if remove_file_if_exists(&footprint_path)? {
        deleted.footprints += 1;
    }

    if let Some(lcsc_part) = lcsc_part {
        deleted.models += delete_model_files_for_lcsc(output_path, library_name, lcsc_part)?;
        if remove_checkpoint_entry(output_path, lcsc_part)? {
            deleted.checkpoint_entries += 1;
        }
    }

    Ok(deleted)
}

fn delete_model_files_for_lcsc(
    output_path: &Path,
    library_name: &str,
    lcsc_part: &str,
) -> Result<usize, String> {
    let shapes_dir = output_path.join(format!("{library_name}.3dshapes"));
    let Ok(entries) = fs::read_dir(&shapes_dir) else {
        return Ok(0);
    };
    let suffix = format!("_{lcsc_part}");
    let mut removed = 0usize;

    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "Failed to read 3D model directory {}: {}",
                shapes_dir.display(),
                err
            )
        })?;
        let path = entry.path();
        if !is_generated_model_for_lcsc(&path, &suffix) {
            continue;
        }
        if remove_file_if_exists(&path)? {
            removed += 1;
        }
    }

    Ok(removed)
}

fn is_generated_model_for_lcsc(path: &Path, lcsc_suffix: &str) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    if !matches!(extension.to_ascii_lowercase().as_str(), "step" | "wrl") {
        return false;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(lcsc_suffix))
}

fn remove_checkpoint_entry(output_path: &Path, lcsc_part: &str) -> Result<bool, String> {
    let checkpoint_path = output_path.join(".checkpoint");
    let content = match fs::read_to_string(&checkpoint_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(format!(
                "Failed to read {}: {}",
                checkpoint_path.display(),
                err
            ));
        }
    };

    let mut removed = false;
    let mut kept_lines = Vec::new();
    for line in content.lines() {
        let checkpoint_id = line
            .split_once('\t')
            .map(|(id, _)| id)
            .unwrap_or(line)
            .trim();
        if checkpoint_id == lcsc_part {
            removed = true;
        } else {
            kept_lines.push(line);
        }
    }

    if !removed {
        return Ok(false);
    }

    let mut updated = kept_lines.join("\n");
    if !updated.is_empty() {
        updated.push('\n');
    }
    fs::write(&checkpoint_path, updated)
        .map_err(|err| format!("Failed to write {}: {}", checkpoint_path.display(), err))?;
    Ok(true)
}

fn remove_file_if_exists(path: &Path) -> Result<bool, String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(format!("Failed to remove {}: {}", path.display(), err)),
    }
}

fn format_delete_result(
    symbol_name: &str,
    source_file: &str,
    deleted_assets: &DeletedGeneratedAssets,
) -> String {
    let mut details = Vec::new();
    if deleted_assets.footprints > 0 {
        details.push(format!("{} footprint", deleted_assets.footprints));
    }
    if deleted_assets.models > 0 {
        details.push(format!("{} 3D model file(s)", deleted_assets.models));
    }
    if deleted_assets.checkpoint_entries > 0 {
        details.push("checkpoint entry".to_string());
    }

    if details.is_empty() {
        format!("Deleted {symbol_name} from {source_file}")
    } else {
        format!(
            "Deleted {symbol_name} from {source_file}; also removed {}",
            details.join(", ")
        )
    }
}

fn line_start(content: &str, index: usize) -> usize {
    content[..index].rfind('\n').map(|pos| pos + 1).unwrap_or(0)
}

fn is_kicad_symbol_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("kicad_sym"))
}

#[cfg(test)]
mod tests {
    use super::{
        ImportedSymbol, ImportedSymbolDeleteRequest, ImportedSymbolUpdateRequest,
        delete_imported_symbol, load_imported_symbols, parse_kicad_symbol_lib, unique_lcsc_parts,
        update_imported_symbol,
    };
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
        let default_child = format!("{symbol_name}_0_1");
        symbol_block_with_children(symbol_name, lcsc_part, &[default_child.as_str()])
    }

    fn symbol_block_with_children(
        symbol_name: &str,
        lcsc_part: Option<&str>,
        child_names: &[&str],
    ) -> String {
        let lcsc_property = lcsc_part
            .map(|part| {
                format!(
                    "    (property\n      \"LCSC Part\"\n      \"{}\"\n      (id 5)\n      (at 0 0 0)\n      (effects (font (size 1.27 1.27) ) hide)\n    )\n",
                    part
                )
            })
            .unwrap_or_default();
        let child_blocks = child_names
            .iter()
            .map(|child_name| format!("    (symbol \"{}\"\n    )\n", child_name))
            .collect::<String>();

        format!(
            "  (symbol \"{}\"\n    (property\n      \"Reference\"\n      \"U\"\n      (id 0)\n      (at 0 0 0)\n      (effects (font (size 1.27 1.27) ) )\n    )\n{}{}  )\n",
            symbol_name, lcsc_property, child_blocks
        )
    }

    fn wrap_library(blocks: &[String]) -> String {
        format!(
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator seex-test)\n{}\n)\n",
            blocks.join("")
        )
    }

    #[test]
    fn parses_minimal_symbol_library_with_source_file() {
        let content = wrap_library(&[symbol_block("Device_C123", Some("C123"))]);

        let parsed =
            parse_kicad_symbol_lib(&content, "alpha.kicad_sym").expect("parse should succeed");

        assert_eq!(
            parsed,
            vec![ImportedSymbol {
                lcsc_part: "C123".to_string(),
                symbol_name: "Device_C123".to_string(),
                source_file: "alpha.kicad_sym".to_string(),
            }]
        );
    }

    #[test]
    fn ignores_symbols_without_lcsc_part() {
        let content = wrap_library(&[
            symbol_block("Device_C123", Some("C123")),
            symbol_block("Graphic_Only", None),
        ]);

        let parsed =
            parse_kicad_symbol_lib(&content, "alpha.kicad_sym").expect("parse should succeed");

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
                    source_file: "alpha.kicad_sym".to_string(),
                },
                ImportedSymbol {
                    lcsc_part: "C123".to_string(),
                    symbol_name: "Device_C123".to_string(),
                    source_file: "beta.kicad_sym".to_string(),
                },
                ImportedSymbol {
                    lcsc_part: "C456".to_string(),
                    symbol_name: "Amplifier_C456".to_string(),
                    source_file: "alpha.kicad_sym".to_string(),
                },
                ImportedSymbol {
                    lcsc_part: "C789".to_string(),
                    symbol_name: "Switch_C789".to_string(),
                    source_file: "beta.kicad_sym".to_string(),
                },
            ]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn updates_symbol_name_lcsc_part_and_child_symbol_names() {
        let root = test_root("update");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[
                symbol_block_with_children(
                    "Alpha",
                    Some("C123"),
                    &["Alpha_0_1", "Alpha_1_1", "AlphaAlias"],
                ),
                symbol_block("Beta", Some("C456")),
            ]),
        )
        .unwrap();

        let result = update_imported_symbol(
            &root,
            ImportedSymbolUpdateRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                new_symbol_name: "Gamma".to_string(),
                lcsc_part: "C999".to_string(),
            },
        )
        .expect("update should succeed");

        assert!(result.contains("Updated Alpha"));
        let updated = fs::read_to_string(&library).unwrap();
        assert!(updated.contains("(symbol \"Gamma\""));
        assert!(updated.contains("(symbol \"Gamma_0_1\""));
        assert!(updated.contains("(symbol \"Gamma_1_1\""));
        assert!(updated.contains("(symbol \"AlphaAlias\""));
        assert!(updated.contains("\"LCSC Part\"\n      \"C999\""));
        assert!(!updated.contains("(symbol \"Alpha\""));
        assert!(!updated.contains("(symbol \"Alpha_0_1\""));
        assert!(!updated.contains("(symbol \"Alpha_1_1\""));

        let response = load_imported_symbols(&root).unwrap();
        assert!(
            response
                .items
                .iter()
                .any(|item| item.symbol_name == "Gamma" && item.lcsc_part == "C999")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_symbol_name_updates() {
        let root = test_root("duplicate_name");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[
                symbol_block("Alpha", Some("C123")),
                symbol_block("Beta", Some("C456")),
            ]),
        )
        .unwrap();

        let error = update_imported_symbol(
            &root,
            ImportedSymbolUpdateRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                new_symbol_name: "Beta".to_string(),
                lcsc_part: "C123".to_string(),
            },
        )
        .unwrap_err();

        assert!(error.contains("already exists"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_source_file_outside_output_directory() {
        let root = test_root("outside");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("alpha.kicad_sym"),
            wrap_library(&[symbol_block("Alpha", Some("C123"))]),
        )
        .unwrap();

        let error = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "../alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                lcsc_part: None,
            },
        )
        .unwrap_err();

        assert!(error.contains("inside the nlbn output directory"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_non_library_source_files() {
        let root = test_root("non_library");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("alpha.txt"), "not a symbol library").unwrap();

        let error = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "alpha.txt".to_string(),
                symbol_name: "Alpha".to_string(),
                lcsc_part: None,
            },
        )
        .unwrap_err();

        assert!(error.contains(".kicad_sym"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deletes_only_target_symbol_block() {
        let root = test_root("delete");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[
                symbol_block("Alpha", Some("C123")),
                symbol_block("Beta", Some("C456")),
            ]),
        )
        .unwrap();

        let result = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha".to_string(),
                lcsc_part: None,
            },
        )
        .expect("delete should succeed");

        assert!(result.contains("Deleted Alpha"));
        let updated = fs::read_to_string(&library).unwrap();
        assert!(!updated.contains("(symbol \"Alpha\""));
        assert!(updated.contains("(symbol \"Beta\""));

        let response = load_imported_symbols(&root).unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].symbol_name, "Beta");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_removes_matching_generated_assets_and_checkpoint_entry() {
        let root = test_root("delete_assets");
        let pretty_dir = root.join("seex.pretty");
        let shapes_dir = root.join("seex.3dshapes");
        fs::create_dir_all(&pretty_dir).unwrap();
        fs::create_dir_all(&shapes_dir).unwrap();

        let symbol_name = "ESP32-C3-MINI-1-N4_C2838502";
        let footprint_path = pretty_dir.join(format!("{symbol_name}.kicad_mod"));
        let step_path = shapes_dir.join("WIFIM-SMD_ESP32-C3-MINI-1_C2838502.step");
        let wrl_path = shapes_dir.join("WIFIM-SMD_ESP32-C3-MINI-1_C2838502.wrl");
        let other_model_path = shapes_dir.join("OTHER_C123.step");

        fs::write(&footprint_path, "footprint").unwrap();
        fs::write(&step_path, "step").unwrap();
        fs::write(&wrl_path, "wrl").unwrap();
        fs::write(&other_model_path, "other").unwrap();
        fs::write(root.join(".checkpoint"), "C123\tsfm\nC2838502\tsfm\n").unwrap();
        fs::write(
            root.join("seex.kicad_sym"),
            wrap_library(&[format!(
                "  (symbol \"{symbol_name}\"\n    (property \"LCSC Part\" \"C2838502\" (id 5) (at 0 0 0))\n    (property \"Footprint\" \"seex:{symbol_name}\" (id 2) (at 0 0 0))\n    (symbol \"{symbol_name}_0_1\")\n  )\n"
            )]),
        )
        .unwrap();

        let result = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "seex.kicad_sym".to_string(),
                symbol_name: symbol_name.to_string(),
                lcsc_part: None,
            },
        )
        .expect("delete should succeed");

        assert!(result.contains("1 footprint"));
        assert!(result.contains("2 3D model file(s)"));
        assert!(result.contains("checkpoint entry"));
        assert!(!footprint_path.exists());
        assert!(!step_path.exists());
        assert!(!wrl_path.exists());
        assert!(other_model_path.exists());
        assert_eq!(
            fs::read_to_string(root.join(".checkpoint")).unwrap(),
            "C123\tsfm\n"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_uses_only_top_level_symbol_names() {
        let root = test_root("delete_nested_name");
        fs::create_dir_all(&root).unwrap();
        let library = root.join("alpha.kicad_sym");
        fs::write(
            &library,
            wrap_library(&[symbol_block_with_children(
                "Alpha",
                Some("C123"),
                &["Alpha_0_1", "Alpha_1_1"],
            )]),
        )
        .unwrap();

        let error = delete_imported_symbol(
            &root,
            ImportedSymbolDeleteRequest {
                source_file: "alpha.kicad_sym".to_string(),
                symbol_name: "Alpha_0_1".to_string(),
                lcsc_part: None,
            },
        )
        .unwrap_err();

        assert!(error.contains("was not found"));
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
                source_file: "alpha.kicad_sym".to_string(),
            },
            ImportedSymbol {
                lcsc_part: "C123".to_string(),
                symbol_name: "Beta".to_string(),
                source_file: "beta.kicad_sym".to_string(),
            },
            ImportedSymbol {
                lcsc_part: "C456".to_string(),
                symbol_name: "Gamma".to_string(),
                source_file: "beta.kicad_sym".to_string(),
            },
        ]);

        assert_eq!(parts, vec!["C123".to_string(), "C456".to_string()]);
    }
}
