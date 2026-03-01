use serde::Serialize;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

const RECIPE_VERSION: u8 = 1;
const FLAG_ASCII_ONLY: u8 = 0x01;
const FLAG_MINIFIED: u8 = 0x02;

#[derive(Debug, Default)]
struct Cli {
    input: PathBuf,
    out_dir: PathBuf,
    chunk_size: usize,
    max_chunks: usize,
    minify: bool,
    stdout: bool,
    prefix: String, // "stele" for .txt, "pktdef" for .json
}

#[derive(Debug, Serialize)]
struct Manifest {
    recipe_version: u8,
    chunk_size: usize,
    chunk_count: usize,
    recipe_len: usize,
    recipe_crc32: u32,
    flags: u8,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = parse_args()?;
    if cli.chunk_size == 0 || cli.chunk_size > 251 {
        return Err("--chunk-size must be in 1..=251".to_string());
    }

    let raw = fs::read_to_string(&cli.input)
        .map_err(|e| format!("failed to read '{}': {e}", cli.input.display()))?;

    ensure_ascii(&raw)?;

    let recipe_text = if cli.minify {
        strip_comments(&raw)
    } else {
        raw
    };

    let recipe_bytes = recipe_text.as_bytes();
    let chunk_count = recipe_bytes.len().div_ceil(cli.chunk_size);
    if chunk_count > cli.max_chunks {
        return Err(format!(
            "chunk_count {chunk_count} exceeds --max-chunks {}",
            cli.max_chunks
        ));
    }

    let manifest = Manifest {
        recipe_version: RECIPE_VERSION,
        chunk_size: cli.chunk_size,
        chunk_count,
        recipe_len: recipe_bytes.len(),
        recipe_crc32: crc32(recipe_bytes),
        flags: if cli.minify {
            FLAG_ASCII_ONLY | FLAG_MINIFIED
        } else {
            FLAG_ASCII_ONLY
        },
    };

    let header = build_header(&manifest, recipe_bytes, &cli.prefix);

    if cli.stdout {
        io::stdout()
            .write_all(header.as_bytes())
            .map_err(|e| format!("failed to write to stdout: {e}"))?;
        return Ok(());
    }

    fs::create_dir_all(&cli.out_dir).map_err(|e| {
        format!(
            "failed to create output dir '{}': {e}",
            cli.out_dir.display()
        )
    })?;

    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("failed to serialize manifest: {e}"))?;

    let header_path = cli.out_dir.join(format!("{}.h", cli.prefix));
    fs::write(&header_path, header)
        .map_err(|e| format!("failed to write '{}': {e}", header_path.display()))?;

    eprintln!("generated:");
    eprintln!("  {}", header_path.display());
    eprintln!("manifest:");
    eprintln!("{manifest_json}");
    eprintln!(
        "recipe_len={} chunk_count={}",
        manifest.recipe_len, manifest.chunk_count
    );
    Ok(())
}

fn parse_args() -> Result<Cli, String> {
    let mut args = env::args().skip(1);
    let mut cli = Cli {
        input: PathBuf::new(),
        out_dir: PathBuf::from("out"),
        chunk_size: 248,
        max_chunks: 120,
        minify: false,
        stdout: false,
        prefix: String::new(),
    };

    let mut saw_input = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--out-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --out-dir".to_string())?;
                cli.out_dir = PathBuf::from(value);
            }
            "--chunk-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --chunk-size".to_string())?;
                cli.chunk_size = value
                    .parse::<usize>()
                    .map_err(|_| "--chunk-size must be integer".to_string())?;
            }
            "--max-chunks" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-chunks".to_string())?;
                cli.max_chunks = value
                    .parse::<usize>()
                    .map_err(|_| "--max-chunks must be integer".to_string())?;
            }
            "--minify" => {
                cli.minify = true;
            }
            "--stdout" => {
                cli.stdout = true;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ if arg.starts_with('-') => {
                return Err(format!("unknown option: {arg}"));
            }
            _ => {
                if saw_input {
                    return Err("multiple input paths are not allowed".to_string());
                }
                saw_input = true;
                cli.input = PathBuf::from(arg);
            }
        }
    }

    if !saw_input {
        return Err("input recipe.txt is required".to_string());
    }

    cli.prefix = match cli.input.extension().and_then(|e| e.to_str()) {
        Some("json") => "pktdef".to_string(),
        _ => "stele".to_string(),
    };

    Ok(cli)
}

fn print_help() {
    println!("stele v0.2.0 -- recipe text / packet-definition JSON to C header converter");
    println!();
    println!("Usage:");
    println!(
        "  stele <input.txt|input.json> -o <out_dir> [--chunk-size 248] [--max-chunks 120] [--minify]"
    );
    println!("  stele <input.txt|input.json> --stdout [--chunk-size 248] [--minify]");
    println!();
    println!("Input extension determines the C identifier prefix:");
    println!("  .txt   -> stele / STELE   (output: stele.h,  load function: load_stele)");
    println!("  .json  -> pktdef / PKTDEF (output: pktdef.h, load function: load_pktdef)");
    println!();
    println!("Also prints manifest metadata (version, sizes, CRC32) to stderr.");
    println!();
    println!("Options:");
    println!("  -o, --out-dir <dir>   Output directory (default: out)");
    println!("  --stdout              Print header to stdout (no files written)");
    println!("  --chunk-size <n>      Chunk size in bytes, 1..=251 (default: 248)");
    println!("  --max-chunks <n>      Maximum number of chunks (default: 120)");
    println!("  --minify              Strip comment lines and collapse blank lines");
    println!("  -h, --help            Show this help");
}

/// Strip comment lines (starting with ';') and collapse runs of blank lines
/// into a single blank line. Preserves all non-comment content exactly.
fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_blank = false;

    for line in input.lines() {
        let trimmed = line.trim();

        // Skip comment lines
        if trimmed.starts_with(';') {
            continue;
        }

        // Collapse consecutive blank lines
        if trimmed.is_empty() {
            if !prev_blank && !out.is_empty() {
                out.push('\n');
                prev_blank = true;
            }
            continue;
        }

        prev_blank = false;
        out.push_str(line);
        out.push('\n');
    }

    // Remove trailing blank line
    while out.ends_with("\n\n") {
        out.pop();
    }

    out
}

fn ensure_ascii(s: &str) -> Result<(), String> {
    for (i, b) in s.bytes().enumerate() {
        if !b.is_ascii() {
            return Err(format!(
                "non-ASCII byte 0x{b:02X} at offset {i}"
            ));
        }
    }
    Ok(())
}

/// Build the C header.
/// `prefix` is the lowercase identifier root ("stele" or "pktdef").
/// The uppercase variant is derived automatically.
fn build_header(manifest: &Manifest, recipe_bytes: &[u8], prefix: &str) -> String {
    let up = prefix.to_uppercase();
    let mut out = String::new();

    out.push_str(&format!("#ifndef {up}_H\n"));
    out.push_str(&format!("#define {up}_H\n\n"));
    out.push_str("#include <stdint.h>\n\n");
    out.push_str("#ifdef __cplusplus\n");
    out.push_str("extern \"C\" {\n");
    out.push_str("#endif\n\n");

    out.push_str(&format!(
        "#define {up}_RECIPE_VERSION {}\n",
        manifest.recipe_version
    ));
    out.push_str(&format!(
        "#define {up}_RECIPE_CHUNK_SIZE {}\n",
        manifest.chunk_size
    ));
    out.push_str(&format!(
        "#define {up}_RECIPE_CHUNK_COUNT {}\n",
        manifest.chunk_count
    ));
    out.push_str(&format!(
        "#define {up}_RECIPE_LENGTH ((uint32_t){})\n",
        manifest.recipe_len
    ));
    out.push_str(&format!(
        "#define {up}_RECIPE_CRC32 0x{:08X}u\n\n",
        manifest.recipe_crc32
    ));

    out.push_str(&format!(
        "static const uint8_t {prefix}_recipe_data[{}] = {{\n",
        recipe_bytes.len()
    ));
    for (i, line_bytes) in recipe_bytes.chunks(16).enumerate() {
        out.push_str("    ");
        for (j, b) in line_bytes.iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("0x{b:02X}"));
        }
        if (i + 1) * 16 < recipe_bytes.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("};\n\n");

    // Align continuation args with the opening parenthesis of the function.
    let fn_indent = " ".repeat("static inline void load_".len() + prefix.len() + 1);
    out.push_str(&format!("static inline void load_{prefix}(const uint16_t chunk,\n"));
    out.push_str(&format!("{fn_indent}const uint16_t max_data_length,\n"));
    out.push_str(&format!("{fn_indent}char *data,\n"));
    out.push_str(&format!("{fn_indent}uint16_t *output_length) {{\n"));
    out.push_str("    if (data == 0 || output_length == 0 || max_data_length == 0) {\n");
    out.push_str("        if (output_length != 0) {\n");
    out.push_str("            *output_length = 0;\n");
    out.push_str("        }\n");
    out.push_str("        return;\n");
    out.push_str("    }\n");
    out.push_str("    const uint32_t start = (uint32_t)chunk * (uint32_t)max_data_length;\n");
    out.push_str(&format!("    if (start >= {up}_RECIPE_LENGTH) {{\n"));
    out.push_str("        *output_length = 0;\n");
    out.push_str("        return;\n");
    out.push_str("    }\n");
    out.push_str(&format!(
        "    uint32_t available = {up}_RECIPE_LENGTH - start;\n"
    ));
    out.push_str("    const uint16_t n = (available < (uint32_t)max_data_length) ? (uint16_t)available : max_data_length;\n");
    out.push_str("    uint16_t i;\n");
    out.push_str("    for (i = 0; i < n; ++i) {\n");
    out.push_str(&format!(
        "        data[i] = (char){prefix}_recipe_data[start + i];\n"
    ));
    out.push_str("    }\n");
    out.push_str("    *output_length = n;\n");
    out.push_str("}\n\n");

    out.push_str("#ifdef __cplusplus\n");
    out.push_str("}\n");
    out.push_str("#endif\n\n");
    out.push_str(&format!("#endif  /* {up}_H */\n"));
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for byte in bytes {
        let mut x = (crc ^ (*byte as u32)) & 0xFF;
        for _ in 0..8 {
            x = if x & 1 != 0 {
                0xEDB8_8320 ^ (x >> 1)
            } else {
                x >> 1
            };
        }
        crc = (crc >> 8) ^ x;
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn ensure_ascii_accepts_ascii() {
        assert!(ensure_ascii("hello world\n").is_ok());
    }

    #[test]
    fn ensure_ascii_rejects_non_ascii() {
        // U+00E9 is 'é', multi-byte UTF-8: 0xC3 0xA9
        assert!(ensure_ascii("caf\u{00e9}").is_err());
    }

    #[test]
    fn strip_comments_removes_comment_lines() {
        let input = "; comment\nAPP test\n; another\nTITLE foo\n";
        let result = strip_comments(input);
        assert_eq!(result, "APP test\nTITLE foo\n");
    }

    #[test]
    fn strip_comments_collapses_blank_lines() {
        let input = "AAA\n\n\n\nBBB\n";
        let result = strip_comments(input);
        assert_eq!(result, "AAA\n\nBBB\n");
    }

    #[test]
    fn strip_comments_handles_indented_comments() {
        let input = "LINE1\n  ; indented comment\nLINE2\n";
        let result = strip_comments(input);
        assert_eq!(result, "LINE1\nLINE2\n");
    }

    #[test]
    fn strip_comments_preserves_semicolons_in_content() {
        // A line where ; appears but is NOT the first non-space char
        let input = "FONT Inter, system-ui, sans-serif\n";
        let result = strip_comments(input);
        assert_eq!(result, "FONT Inter, system-ui, sans-serif\n");
    }

    #[test]
    fn strip_comments_empty_input() {
        assert_eq!(strip_comments(""), "");
        assert_eq!(strip_comments("; only comments\n; here\n"), "");
    }
}
