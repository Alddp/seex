use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const DEFAULT_NICKNAME: &str = "personal";

#[derive(Debug)]
struct Config {
    source: PathBuf,
    library_root: PathBuf,
    nickname: String,
    model_path_mode: ModelPathMode,
    dry_run: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelPathMode {
    Absolute,
    Kiprjmod,
}

#[derive(Debug)]
struct ImportPlan {
    symbol_files: Vec<PathBuf>,
    footprint_files: Vec<PathBuf>,
    model_files: Vec<PathBuf>,
    symbol_library: PathBuf,
    footprint_dir: PathBuf,
    model_dir: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args().skip(1))?;
    let plan = ImportPlan::discover(&config)?;

    if plan.symbol_files.is_empty() && plan.footprint_files.is_empty() {
        return Err(format!(
            "no KiCad symbol or footprint files found under {}",
            config.source.display()
        ));
    }

    print_plan(&config, &plan);
    if config.dry_run {
        return Ok(());
    }

    fs::create_dir_all(&config.library_root)
        .map_err(|error| io_context("create library root", &config.library_root, error))?;
    fs::create_dir_all(&plan.footprint_dir)
        .map_err(|error| io_context("create footprint dir", &plan.footprint_dir, error))?;
    fs::create_dir_all(&plan.model_dir)
        .map_err(|error| io_context("create model dir", &plan.model_dir, error))?;

    copy_models(&plan)?;
    import_footprints(&config, &plan)?;
    import_symbols(&config, &plan)?;

    println!("done");
    Ok(())
}

impl Config {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut source = None;
        let mut library_root = None;
        let mut nickname = DEFAULT_NICKNAME.to_string();
        let mut model_path_mode = ModelPathMode::Absolute;
        let mut dry_run = false;

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                "--library-root" => {
                    library_root = Some(expand_user_path(
                        &args
                            .next()
                            .ok_or("--library-root requires a path".to_string())?,
                    ));
                }
                "--nickname" => {
                    nickname = args
                        .next()
                        .ok_or("--nickname requires a value".to_string())?;
                    validate_nickname(&nickname)?;
                }
                "--model-path-mode" => {
                    let value = args
                        .next()
                        .ok_or("--model-path-mode requires absolute or kiprjmod".to_string())?;
                    model_path_mode = match value.as_str() {
                        "absolute" => ModelPathMode::Absolute,
                        "kiprjmod" => ModelPathMode::Kiprjmod,
                        _ => {
                            return Err(
                                "--model-path-mode must be either absolute or kiprjmod".to_string()
                            );
                        }
                    };
                }
                "--dry-run" => dry_run = true,
                value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
                value => {
                    if source.is_some() {
                        return Err(format!("unexpected extra argument: {value}"));
                    }
                    source = Some(expand_user_path(value));
                }
            }
        }

        let source = source.ok_or("missing source vendor package path".to_string())?;
        let library_root = library_root.unwrap_or_else(default_library_root);

        Ok(Self {
            source,
            library_root,
            nickname,
            model_path_mode,
            dry_run,
        })
    }
}

impl ImportPlan {
    fn discover(config: &Config) -> Result<Self, String> {
        if !config.source.is_dir() {
            return Err(format!(
                "source is not a directory: {}",
                config.source.display()
            ));
        }

        let mut files = Vec::new();
        collect_files(&config.source, &mut files)
            .map_err(|error| io_context("scan source", &config.source, error))?;

        let symbol_files = files
            .iter()
            .filter(|path| path.extension() == Some(OsStr::new("kicad_sym")))
            .cloned()
            .collect::<Vec<_>>();
        let footprint_files = files
            .iter()
            .filter(|path| path.extension() == Some(OsStr::new("kicad_mod")))
            .cloned()
            .collect::<Vec<_>>();
        let model_files = files
            .iter()
            .filter(|path| is_model_file(path))
            .cloned()
            .collect::<Vec<_>>();

        let symbol_library = config
            .library_root
            .join(format!("{}.kicad_sym", config.nickname));
        let footprint_dir = config
            .library_root
            .join(format!("{}.pretty", config.nickname));
        let model_dir = config
            .library_root
            .join(format!("{}.3dshapes", config.nickname));

        Ok(Self {
            symbol_files,
            footprint_files,
            model_files,
            symbol_library,
            footprint_dir,
            model_dir,
        })
    }
}

fn print_help() {
    println!("kicad-vendor-import");
    println!();
    println!(
        "Import third-party KiCad symbol, footprint, and 3D model files into one personal library."
    );
    println!();
    println!("Usage:");
    println!("  kicad-vendor-import <vendor-package-dir> [options]");
    println!();
    println!("Options:");
    println!(
        "  --library-root <PATH>       Target library root [default: ~/Documents/KiCad/libs/personal]"
    );
    println!("  --nickname <NAME>           KiCad library nickname [default: personal]");
    println!("  --model-path-mode <MODE>    absolute or kiprjmod [default: absolute]");
    println!("  --dry-run                   Print planned actions without writing files");
}

fn print_plan(config: &Config, plan: &ImportPlan) {
    println!("source: {}", config.source.display());
    println!("target symbol library: {}", plan.symbol_library.display());
    println!("target footprint dir: {}", plan.footprint_dir.display());
    println!("target model dir: {}", plan.model_dir.display());
    println!("symbols: {}", plan.symbol_files.len());
    println!("footprints: {}", plan.footprint_files.len());
    println!("3d models: {}", plan.model_files.len());
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn copy_models(plan: &ImportPlan) -> Result<(), String> {
    for source in &plan.model_files {
        let target = plan.model_dir.join(file_name(source)?);
        fs::copy(source, &target).map_err(|error| io_context("copy 3D model", source, error))?;
    }
    Ok(())
}

fn import_footprints(config: &Config, plan: &ImportPlan) -> Result<(), String> {
    let model_names = plan
        .model_files
        .iter()
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect::<BTreeSet<_>>();

    for source in &plan.footprint_files {
        let target = plan.footprint_dir.join(file_name(source)?);
        let content = fs::read_to_string(source)
            .map_err(|error| io_context("read footprint", source, error))?;
        let rewritten = rewrite_model_paths(&content, config, plan, &model_names);
        fs::write(&target, rewritten)
            .map_err(|error| io_context("write footprint", &target, error))?;
    }

    Ok(())
}

fn import_symbols(config: &Config, plan: &ImportPlan) -> Result<(), String> {
    let footprint_names = plan
        .footprint_files
        .iter()
        .map(|path| footprint_name(path))
        .collect::<Result<BTreeSet<_>, _>>()?;

    let mut existing = if plan.symbol_library.exists() {
        fs::read_to_string(&plan.symbol_library)
            .map_err(|error| io_context("read symbol library", &plan.symbol_library, error))?
    } else {
        "(kicad_symbol_lib (version 20211014) (generator kicad-vendor-import)\n)\n".to_string()
    };

    let mut existing_names = extract_symbol_names(&existing);
    let mut imported = Vec::new();
    for source in &plan.symbol_files {
        let content = fs::read_to_string(source)
            .map_err(|error| io_context("read symbol library", source, error))?;
        for symbol in extract_top_level_symbols(&content)? {
            let name = parse_symbol_name(&symbol)
                .ok_or_else(|| format!("failed to parse symbol name in {}", source.display()))?;
            if existing_names.contains(&name) {
                println!("skip existing symbol: {name}");
                continue;
            }

            existing_names.insert(name);
            imported.push(rewrite_symbol_footprints(
                &symbol,
                &config.nickname,
                &footprint_names,
            ));
        }
    }

    if imported.is_empty() {
        return Ok(());
    }

    existing = append_symbols(&existing, &imported)?;
    fs::write(&plan.symbol_library, existing)
        .map_err(|error| io_context("write symbol library", &plan.symbol_library, error))
}

fn rewrite_model_paths(
    content: &str,
    config: &Config,
    plan: &ImportPlan,
    model_names: &BTreeSet<String>,
) -> String {
    let mut rewritten = String::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("(model ") {
            if let Some(model_name) = model_name_from_model_line(rest, model_names) {
                let indent_len = line.len() - trimmed.len();
                let indent = &line[..indent_len];
                rewritten.push_str(indent);
                rewritten.push_str("(model ");
                rewritten.push_str(&quote_kicad_string(&model_reference(
                    config,
                    plan,
                    &model_name,
                )));
                rewritten.push('\n');
                continue;
            }
        }

        rewritten.push_str(line);
        rewritten.push('\n');
    }
    rewritten
}

fn model_name_from_model_line(rest: &str, model_names: &BTreeSet<String>) -> Option<String> {
    let raw = rest
        .split_whitespace()
        .next()
        .map(|value| value.trim_matches('"'))?;
    let basename = Path::new(raw).file_name()?.to_string_lossy().to_string();
    model_names.contains(&basename).then_some(basename)
}

fn model_reference(config: &Config, plan: &ImportPlan, model_name: &str) -> String {
    match config.model_path_mode {
        ModelPathMode::Absolute => plan.model_dir.join(model_name).display().to_string(),
        ModelPathMode::Kiprjmod => {
            format!("${{KIPRJMOD}}/{}.3dshapes/{model_name}", config.nickname)
        }
    }
}

fn rewrite_symbol_footprints(
    symbol: &str,
    nickname: &str,
    footprint_names: &BTreeSet<String>,
) -> String {
    let mut rewritten = String::new();
    for line in symbol.lines() {
        if line.contains("(property \"Footprint\"") {
            let mut next = line.to_string();
            for footprint in footprint_names {
                let bare = format!("\"{footprint}\"");
                let prefixed = format!("\"{nickname}:{footprint}\"");
                if next.contains(&bare) {
                    next = next.replace(&bare, &prefixed);
                    break;
                }
            }
            rewritten.push_str(&next);
        } else {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    rewritten
}

fn append_symbols(existing: &str, symbols: &[String]) -> Result<String, String> {
    let insert_at = existing
        .rfind(')')
        .ok_or("target symbol library is not a KiCad symbol library".to_string())?;
    let mut output = String::new();
    output.push_str(existing[..insert_at].trim_end());
    output.push('\n');
    for symbol in symbols {
        output.push_str(symbol.trim_end());
        output.push('\n');
    }
    output.push_str(")\n");
    Ok(output)
}

fn extract_top_level_symbols(content: &str) -> Result<Vec<String>, String> {
    let mut symbols = Vec::new();
    let bytes = content.as_bytes();
    let mut index = 0;
    while let Some(relative) = content[index..].find("(symbol ") {
        let start = index + relative;
        let end = find_matching_paren(bytes, start)
            .ok_or("failed to parse KiCad symbol file: unmatched parenthesis".to_string())?;
        symbols.push(content[start..=end].to_string());
        index = end + 1;
    }
    Ok(symbols)
}

fn extract_symbol_names(content: &str) -> BTreeSet<String> {
    extract_top_level_symbols(content)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|symbol| parse_symbol_name(&symbol))
        .collect()
}

fn parse_symbol_name(symbol: &str) -> Option<String> {
    let rest = symbol.strip_prefix("(symbol ")?;
    parse_quoted_or_atom(rest)
}

fn parse_quoted_or_atom(input: &str) -> Option<String> {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        input
            .split_whitespace()
            .next()
            .map(|value| value.trim_end_matches(')').to_string())
    }
}

fn find_matching_paren(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match *byte {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn footprint_name(path: &Path) -> Result<String, String> {
    path.file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| format!("invalid footprint file name: {}", path.display()))
}

fn file_name(path: &Path) -> Result<&OsStr, String> {
    path.file_name()
        .ok_or_else(|| format!("invalid file name: {}", path.display()))
}

fn is_model_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "stp" | "step" | "wrl"))
        .unwrap_or(false)
}

fn quote_kicad_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn validate_nickname(nickname: &str) -> Result<(), String> {
    if nickname.is_empty() {
        return Err("nickname cannot be empty".to_string());
    }

    if nickname
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, ':' | '/' | '\\'))
    {
        return Err("nickname cannot contain whitespace, ':', '/', or '\\'".to_string());
    }

    Ok(())
}

fn default_library_root() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents/KiCad/libs/personal")
}

fn expand_user_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }

    if let Some(stripped) = trimmed.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    }

    PathBuf::from(trimmed)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn io_context(action: &str, path: &Path, error: io::Error) -> String {
    format!("{action}: {}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_footprint_property_to_library_qualified_name() {
        let mut footprints = BTreeSet::new();
        footprints.insert("SOP65P640X120-28N".to_string());

        let rewritten = rewrite_symbol_footprints(
            "(symbol \"BM3451\"\n  (property \"Footprint\" \"SOP65P640X120-28N\" (at 0 0 0))\n)",
            "personal",
            &footprints,
        );

        assert!(rewritten.contains("\"personal:SOP65P640X120-28N\""));
    }

    #[test]
    fn rewrites_model_path_to_absolute_target() {
        let config = Config {
            source: PathBuf::from("/src"),
            library_root: PathBuf::from("/lib"),
            nickname: "personal".to_string(),
            model_path_mode: ModelPathMode::Absolute,
            dry_run: false,
        };
        let plan = ImportPlan {
            symbol_files: Vec::new(),
            footprint_files: Vec::new(),
            model_files: Vec::new(),
            symbol_library: PathBuf::from("/lib/personal.kicad_sym"),
            footprint_dir: PathBuf::from("/lib/personal.pretty"),
            model_dir: PathBuf::from("/lib/personal.3dshapes"),
        };
        let mut models = BTreeSet::new();
        models.insert("BM3451TNDC-T28A.stp".to_string());

        let rewritten = rewrite_model_paths(
            "  (model BM3451TNDC-T28A.stp\n    (at (xyz 0 0 0))\n  )\n",
            &config,
            &plan,
            &models,
        );

        assert!(rewritten.contains("(model \"/lib/personal.3dshapes/BM3451TNDC-T28A.stp\""));
    }

    #[test]
    fn extracts_top_level_symbols() {
        let content = "(kicad_symbol_lib\n  (symbol \"A\" (property \"Value\" \"A\"))\n  (symbol \"B\" (property \"Value\" \"B\"))\n)\n";

        let symbols = extract_top_level_symbols(content).expect("symbols should parse");

        assert_eq!(symbols.len(), 2);
        assert_eq!(parse_symbol_name(&symbols[0]).as_deref(), Some("A"));
        assert_eq!(parse_symbol_name(&symbols[1]).as_deref(), Some("B"));
    }
}
