use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug)]
struct Rgb {
    r: f64,
    g: f64,
    b: f64,
}

#[derive(Clone, Copy, Debug)]
struct Palette {
    name: &'static str,
    foreground: Rgb,
    background: Rgb,
}

#[derive(Clone, Copy, Debug)]
enum Theme {
    CatppuccinMocha,
    ClaudeWarm,
    SepiaDark,
    MidnightBlue,
    ForestGreen,
}

impl Theme {
    fn parse(value: &str) -> Result<Self, String> {
        let normalized = value.to_ascii_lowercase().replace(['_', ' '], "-");
        match normalized.as_str() {
            "mocha" | "catppuccin" | "catppuccin-mocha" => Ok(Self::CatppuccinMocha),
            "claude" | "claude-warm" => Ok(Self::ClaudeWarm),
            "sepia" | "sepia-dark" => Ok(Self::SepiaDark),
            "midnight" | "midnight-blue" => Ok(Self::MidnightBlue),
            "forest" | "forest-green" => Ok(Self::ForestGreen),
            _ => Err(format!(
                "invalid theme '{value}', expected mocha, claude-warm, sepia-dark, midnight-blue, or forest-green"
            )),
        }
    }

    fn palette(self, strength: Strength) -> Palette {
        match self {
            Self::CatppuccinMocha => Palette {
                name: "catppuccin-mocha",
                foreground: strength.foreground(),
                background: mocha("mantle"),
            },
            Self::ClaudeWarm => Palette {
                name: "claude-warm",
                foreground: Rgb::white(),
                background: Rgb::from_u8(42, 37, 34),
            },
            Self::SepiaDark => Palette {
                name: "sepia-dark",
                foreground: Rgb::white(),
                background: Rgb::from_u8(40, 35, 25),
            },
            Self::MidnightBlue => Palette {
                name: "midnight-blue",
                foreground: Rgb::white(),
                background: Rgb::from_u8(25, 30, 45),
            },
            Self::ForestGreen => Palette {
                name: "forest-green",
                foreground: Rgb::white(),
                background: Rgb::from_u8(25, 35, 30),
            },
        }
    }
}

impl Rgb {
    const fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
        }
    }

    const fn white() -> Self {
        Self::from_u8(255, 255, 255)
    }

    fn from_hex(input: &str) -> Result<Self, String> {
        let hex = input.strip_prefix('#').unwrap_or(input);
        if hex.len() != 6 {
            return Err(format!("expected a 6-digit hex color, got '{input}'"));
        }

        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|_| format!("invalid red channel in '{input}'"))?;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|_| format!("invalid green channel in '{input}'"))?;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|_| format!("invalid blue channel in '{input}'"))?;

        Ok(Self {
            r: f64::from(r) / 255.0,
            g: f64::from(g) / 255.0,
            b: f64::from(b) / 255.0,
        })
    }

    fn channel(self, index: usize) -> f64 {
        match index {
            0 => self.r,
            1 => self.g,
            2 => self.b,
            _ => unreachable!("RGB channel index out of range"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Strength {
    Soft,
    Balanced,
    HighContrast,
}

impl Strength {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "soft" => Ok(Self::Soft),
            "balanced" => Ok(Self::Balanced),
            "high-contrast" | "high" => Ok(Self::HighContrast),
            _ => Err(format!(
                "invalid strength '{value}', expected soft, balanced, or high-contrast"
            )),
        }
    }

    fn foreground(self) -> Rgb {
        match self {
            Self::Soft => mocha("subtext1"),
            Self::Balanced => mocha("text"),
            Self::HighContrast => mocha("rosewater"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Background {
    Base,
    Mantle,
    Crust,
}

impl Background {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "base" => Ok(Self::Base),
            "mantle" => Ok(Self::Mantle),
            "crust" => Ok(Self::Crust),
            _ => Err(format!(
                "invalid background '{value}', expected base, mantle, or crust"
            )),
        }
    }

    fn color(self) -> Rgb {
        match self {
            Self::Base => mocha("base"),
            Self::Mantle => mocha("mantle"),
            Self::Crust => mocha("crust"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceMode {
    Auto,
    Light,
    Dark,
}

impl SourceMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "light" | "light-mode" => Ok(Self::Light),
            "dark" | "dark-mode" => Ok(Self::Dark),
            _ => Err(format!(
                "invalid source mode '{value}', expected auto, light, or dark"
            )),
        }
    }
}

#[derive(Debug)]
struct Config {
    input: PathBuf,
    destination: Option<PathBuf>,
    suffix: String,
    overwrite: bool,
    dry_run: bool,
    gs: OsString,
    foreground: Rgb,
    background: Rgb,
    theme_name: &'static str,
    compatibility: String,
    source_mode: SourceMode,
}

#[derive(Debug)]
struct Job {
    input: PathBuf,
    output: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pdfnight: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let config = parse_args(env::args_os().skip(1))?;
    let jobs = plan_jobs(&config)?;

    if jobs.is_empty() {
        return Err("no PDF inputs found".to_string());
    }

    for job in jobs {
        if config.dry_run {
            println!("{} -> {}", job.input.display(), job.output.display());
            continue;
        }

        convert_pdf(&config, &job)?;
        println!("created {}", job.output.display());
    }

    Ok(())
}

fn parse_args<I>(args: I) -> Result<Config, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter().peekable();
    let mut input = None;
    let mut destination = None;
    let mut suffix = None;
    let mut overwrite = false;
    let mut dry_run = false;
    let mut gs = OsString::from("gs");
    let mut foreground_override = None;
    let mut background_override = None;
    let mut strength = Strength::Balanced;
    let mut theme = Theme::CatppuccinMocha;
    let mut compatibility = "1.7".to_string();
    let mut source_mode = SourceMode::Auto;

    while let Some(arg) = args.next() {
        let arg_text = arg.to_string_lossy();
        match arg_text.as_ref() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("pdfnight {VERSION}");
                std::process::exit(0);
            }
            "--suffix" => suffix = Some(take_string(&mut args, "--suffix")?),
            "--overwrite" => overwrite = true,
            "--dry-run" => dry_run = true,
            "--gs" => gs = take_os_string(&mut args, "--gs")?,
            "--theme" => theme = Theme::parse(&take_string(&mut args, "--theme")?)?,
            "--strength" => strength = Strength::parse(&take_string(&mut args, "--strength")?)?,
            "--background" => {
                background_override =
                    Some(Background::parse(&take_string(&mut args, "--background")?)?.color());
            }
            "--foreground" => {
                foreground_override =
                    Some(Rgb::from_hex(&take_string(&mut args, "--foreground")?)?);
            }
            "--compatibility" => compatibility = take_string(&mut args, "--compatibility")?,
            "--source-mode" => {
                source_mode = SourceMode::parse(&take_string(&mut args, "--source-mode")?)?;
            }
            value if value.starts_with('-') => return Err(format!("unknown option '{value}'")),
            _ => {
                if input.is_none() {
                    input = Some(PathBuf::from(arg));
                } else if destination.is_none() {
                    destination = Some(PathBuf::from(arg));
                } else {
                    return Err(format!(
                        "unexpected extra positional argument '{}'",
                        arg_text
                    ));
                }
            }
        }
    }

    let input = input.ok_or_else(|| "missing source PDF".to_string())?;
    let palette = theme.palette(strength);
    let foreground = foreground_override.unwrap_or(palette.foreground);
    let background = background_override.unwrap_or(palette.background);
    let suffix = suffix.unwrap_or_else(|| format!("_{}", palette.name.replace('-', "_")));

    Ok(Config {
        input,
        destination,
        suffix,
        overwrite,
        dry_run,
        gs,
        foreground,
        background,
        theme_name: palette.name,
        compatibility,
        source_mode,
    })
}

fn take_os_string<I>(args: &mut std::iter::Peekable<I>, option: &str) -> Result<OsString, String>
where
    I: Iterator<Item = OsString>,
{
    args.next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn take_string<I>(args: &mut std::iter::Peekable<I>, option: &str) -> Result<String, String>
where
    I: Iterator<Item = OsString>,
{
    let value = take_os_string(args, option)?;
    value
        .into_string()
        .map_err(|_| format!("{option} value must be valid UTF-8"))
}

fn plan_jobs(config: &Config) -> Result<Vec<Job>, String> {
    let input_meta = fs::metadata(&config.input)
        .map_err(|err| format!("cannot read '{}': {err}", config.input.display()))?;

    if !input_meta.is_file() {
        return Err(format!("source is not a file: {}", config.input.display()));
    }

    if !is_pdf(&config.input) {
        return Err(format!("source is not a PDF: {}", config.input.display()));
    }

    let output =
        output_from_destination(&config.input, config.destination.as_deref(), &config.suffix)?;

    if same_path(&config.input, &output) {
        return Err(format!(
            "refusing to write output over source: {}",
            config.input.display()
        ));
    }

    validate_output(&output, config.overwrite, config.dry_run)?;

    Ok(vec![Job {
        input: config.input.clone(),
        output,
    }])
}

fn output_from_destination(
    source: &Path,
    destination: Option<&Path>,
    suffix: &str,
) -> Result<PathBuf, String> {
    let output = match destination {
        Some(destination) => {
            let is_directory_destination = destination.exists() && destination.is_dir()
                || destination
                    .as_os_str()
                    .to_string_lossy()
                    .ends_with(std::path::MAIN_SEPARATOR)
                || destination.extension().is_none();

            if is_directory_destination {
                destination.join(default_output_name(source, suffix)?)
            } else {
                destination.to_path_buf()
            }
        }
        None => default_output_path(source, suffix)?,
    };

    Ok(output)
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
}

fn default_output_name(input: &Path, suffix: &str) -> Result<OsString, String> {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("cannot derive output name from '{}'", input.display()))?;

    Ok(OsString::from(format!("{stem}{suffix}.pdf")))
}

fn default_output_path(input: &Path, suffix: &str) -> Result<PathBuf, String> {
    let mut output = input.to_path_buf();
    output.set_file_name(default_output_name(input, suffix)?);
    Ok(output)
}

fn validate_output(output: &Path, overwrite: bool, dry_run: bool) -> Result<(), String> {
    if output.exists() && !overwrite {
        return Err(format!(
            "output already exists: {} (use --overwrite to replace it)",
            output.display()
        ));
    }

    if dry_run {
        return Ok(());
    }

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "cannot create output directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn convert_pdf(config: &Config, job: &Job) -> Result<(), String> {
    if same_path(&job.input, &job.output) {
        return Err(format!(
            "refusing to write output over input: {}",
            job.input.display()
        ));
    }

    let resolved_mode = resolve_source_mode(config, &job.input)?;
    eprintln!("theme: {}, source mode: {resolved_mode}", config.theme_name);
    let transfer = color_transfer(config.foreground, config.background, resolved_mode);
    let output_arg = format!("-sOutputFile={}", job.output.display());
    let compatibility_arg = format!("-dCompatibilityLevel={}", config.compatibility);

    let status = Command::new(&config.gs)
        .arg("-q")
        .arg("-dBATCH")
        .arg("-dNOPAUSE")
        .arg("-dSAFER")
        .arg("-sDEVICE=pdfwrite")
        .arg(compatibility_arg)
        .arg("-dDownsampleColorImages=false")
        .arg("-dDownsampleGrayImages=false")
        .arg("-dDownsampleMonoImages=false")
        .arg("-dAutoFilterColorImages=false")
        .arg("-dAutoFilterGrayImages=false")
        .arg("-dColorImageFilter=/FlateEncode")
        .arg("-dGrayImageFilter=/FlateEncode")
        .arg("-dMonoImageFilter=/FlateEncode")
        .arg(output_arg)
        .arg("-c")
        .arg(transfer)
        .arg("-f")
        .arg(&job.input)
        .stdin(Stdio::null())
        .status()
        .map_err(|err| {
            format!(
                "failed to run Ghostscript '{}': {err}",
                config.gs.to_string_lossy()
            )
        })?;

    if !status.success() {
        return Err(format!(
            "Ghostscript failed for '{}' with status {status}",
            job.input.display()
        ));
    }

    Ok(())
}

fn resolve_source_mode(config: &Config, input: &Path) -> Result<SourceMode, String> {
    match config.source_mode {
        SourceMode::Auto => detect_source_mode(config, input),
        mode => Ok(mode),
    }
}

fn detect_source_mode(config: &Config, input: &Path) -> Result<SourceMode, String> {
    let sample_dir = TempDir::new("pdfnight-sample")?;
    let output_pattern = sample_dir.path().join("page-%03d.ppm");
    let output_arg = format!("-sOutputFile={}", output_pattern.display());
    let status = Command::new(&config.gs)
        .arg("-q")
        .arg("-dBATCH")
        .arg("-dNOPAUSE")
        .arg("-dSAFER")
        .arg("-sDEVICE=ppmraw")
        .arg("-r12")
        .arg("-dFirstPage=1")
        .arg("-dLastPage=3")
        .arg(output_arg)
        .arg(input)
        .stdin(Stdio::null())
        .status()
        .map_err(|err| {
            format!(
                "failed to run Ghostscript '{}': {err}",
                config.gs.to_string_lossy()
            )
        })?;

    if !status.success() {
        return Err(format!(
            "Ghostscript failed while sampling '{}' with status {status}",
            input.display()
        ));
    }

    let mut samples = Vec::new();
    for entry in fs::read_dir(sample_dir.path()).map_err(|err| {
        format!(
            "cannot read sampling directory '{}': {err}",
            sample_dir.path().display()
        )
    })? {
        let entry = entry.map_err(|err| format!("cannot read sampling output: {err}"))?;
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("ppm") {
            samples.push(sample_ppm_luminance(&entry.path())?);
        }
    }

    if samples.is_empty() {
        return Err("Ghostscript did not produce any page samples".to_string());
    }

    let average_background =
        samples.iter().map(|sample| sample.background).sum::<f64>() / samples.len() as f64;
    let average_page = samples.iter().map(|sample| sample.page).sum::<f64>() / samples.len() as f64;
    let mode = if average_background < 0.45 || average_page < 0.42 {
        SourceMode::Dark
    } else {
        SourceMode::Light
    };

    eprintln!(
        "detected {} PDF: background luminance {:.3}, page luminance {:.3}",
        mode, average_background, average_page
    );

    Ok(mode)
}

#[derive(Debug)]
struct LuminanceSample {
    background: f64,
    page: f64,
}

fn sample_ppm_luminance(path: &Path) -> Result<LuminanceSample, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|err| format!("cannot open sample '{}': {err}", path.display()))?
        .read_to_end(&mut bytes)
        .map_err(|err| format!("cannot read sample '{}': {err}", path.display()))?;

    let (width, height, pixels_start) = parse_ppm_header(&bytes)?;
    let expected_len = pixels_start + width * height * 3;
    if bytes.len() < expected_len {
        return Err(format!("sample '{}' is truncated", path.display()));
    }

    let mut background_sum = 0.0;
    let mut background_count = 0usize;
    let mut page_sum = 0.0;
    let mut page_count = 0usize;
    let border_x = (width / 10).max(1);
    let border_y = (height / 10).max(1);

    for y in 0..height {
        for x in 0..width {
            let offset = pixels_start + (y * width + x) * 3;
            let lum = luminance(bytes[offset], bytes[offset + 1], bytes[offset + 2]);
            page_sum += lum;
            page_count += 1;
            if x < border_x || x >= width - border_x || y < border_y || y >= height - border_y {
                background_sum += lum;
                background_count += 1;
            }
        }
    }

    Ok(LuminanceSample {
        background: background_sum / background_count as f64,
        page: page_sum / page_count as f64,
    })
}

fn parse_ppm_header(bytes: &[u8]) -> Result<(usize, usize, usize), String> {
    let mut index = 0;
    let magic = next_ppm_token(bytes, &mut index)?;
    if magic != "P6" {
        return Err("sample is not a raw PPM image".to_string());
    }
    let width = next_ppm_token(bytes, &mut index)?
        .parse::<usize>()
        .map_err(|_| "invalid PPM width".to_string())?;
    let height = next_ppm_token(bytes, &mut index)?
        .parse::<usize>()
        .map_err(|_| "invalid PPM height".to_string())?;
    let max = next_ppm_token(bytes, &mut index)?
        .parse::<usize>()
        .map_err(|_| "invalid PPM max value".to_string())?;
    if max != 255 {
        return Err("unsupported PPM max value".to_string());
    }

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    Ok((width, height, index))
}

fn next_ppm_token(bytes: &[u8], index: &mut usize) -> Result<String, String> {
    loop {
        while *index < bytes.len() && bytes[*index].is_ascii_whitespace() {
            *index += 1;
        }
        if *index < bytes.len() && bytes[*index] == b'#' {
            while *index < bytes.len() && bytes[*index] != b'\n' {
                *index += 1;
            }
            continue;
        }
        break;
    }

    let start = *index;
    while *index < bytes.len() && !bytes[*index].is_ascii_whitespace() {
        *index += 1;
    }

    if start == *index {
        return Err("unexpected end of PPM header".to_string());
    }

    String::from_utf8(bytes[start..*index].to_vec())
        .map_err(|_| "PPM header is not valid UTF-8".to_string())
}

fn luminance(red: u8, green: u8, blue: u8) -> f64 {
    (0.2126 * f64::from(red) + 0.7152 * f64::from(green) + 0.0722 * f64::from(blue)) / 255.0
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Result<Self, String> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_millis();
        let path = env::temp_dir().join(format!("{prefix}-{}-{millis}", std::process::id()));
        fs::create_dir(&path).map_err(|err| {
            format!(
                "cannot create temporary directory '{}': {err}",
                path.display()
            )
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn color_transfer(foreground: Rgb, background: Rgb, source_mode: SourceMode) -> String {
    let mut pieces = Vec::with_capacity(4);
    for index in [0, 1, 2, 0] {
        let bg = background.channel(index);
        let fg = foreground.channel(index);
        let function = match source_mode {
            SourceMode::Auto => unreachable!("source mode must be resolved before conversion"),
            SourceMode::Light => format!("{{ 1 exch sub {:.6} mul {:.6} add }}", fg - bg, bg),
            SourceMode::Dark => format!("{{ {:.6} mul {:.6} add }}", fg - bg, bg),
        };
        pieces.push(function);
    }

    format!("{} setcolortransfer", pieces.join(" "))
}

impl std::fmt::Display for SourceMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            SourceMode::Auto => "auto",
            SourceMode::Light => "light",
            SourceMode::Dark => "dark",
        };
        formatter.write_str(value)
    }
}

fn mocha(name: &str) -> Rgb {
    let hex = match name {
        "rosewater" => "f5e0dc",
        "text" => "cdd6f4",
        "subtext1" => "bac2de",
        "base" => "1e1e2e",
        "mantle" => "181825",
        "crust" => "11111b",
        _ => unreachable!("unknown Catppuccin Mocha color"),
    };

    Rgb::from_hex(hex).expect("hard-coded Catppuccin color must be valid")
}

fn print_help() {
    println!(
        "\
pdfnight {VERSION}

Convert PDFs to a dark-mode palette using Ghostscript.

USAGE:
    pdfnight [OPTIONS] <SOURCE_FILE> [DESTINATION_LOCATION]

OPTIONS:
        --suffix <TEXT>          Output suffix before .pdf [default: theme name]
        --overwrite              Replace existing output PDFs
        --dry-run                Print planned conversions without writing files
        --gs <COMMAND>           Ghostscript command [default: gs]
        --theme <THEME>          mocha, claude-warm, sepia-dark, midnight-blue, forest-green
                                  [default: mocha]
        --strength <MODE>        soft, balanced, high-contrast [default: balanced]
        --background <TONE>      base, mantle, crust [default: mantle]
        --foreground <HEX>       Override foreground color, e.g. cdd6f4
        --source-mode <MODE>     auto, light, dark [default: auto]
        --compatibility <VER>    Ghostscript PDF compatibility [default: 1.7]
    -h, --help                   Print help
    -V, --version                Print version

EXAMPLES:
    pdfnight book.pdf
    pdfnight book.pdf converted/
    pdfnight book.pdf converted/book-dark.pdf
    pdfnight --theme forest-green book.pdf
    pdfnight --theme midnight-blue book.pdf
    pdfnight --source-mode dark already-dark.pdf
    pdfnight --background crust --strength high-contrast book.pdf
"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors() {
        let color = Rgb::from_hex("#cdd6f4").unwrap();
        assert!((color.r - 0.803922).abs() < 0.00001);
        assert!((color.g - 0.839216).abs() < 0.00001);
        assert!((color.b - 0.956863).abs() < 0.00001);
    }

    #[test]
    fn parses_theme_aliases() {
        assert!(matches!(
            Theme::parse("catppuccin-mocha").unwrap(),
            Theme::CatppuccinMocha
        ));
        assert!(matches!(
            Theme::parse("claude_warm").unwrap(),
            Theme::ClaudeWarm
        ));
        assert!(matches!(
            Theme::parse("midnight blue").unwrap(),
            Theme::MidnightBlue
        ));
    }

    #[test]
    fn uses_online_tool_backgrounds_for_added_themes() {
        let claude = Theme::ClaudeWarm.palette(Strength::Balanced);
        assert!((claude.background.r - 42.0 / 255.0).abs() < 0.00001);
        assert!((claude.background.g - 37.0 / 255.0).abs() < 0.00001);
        assert!((claude.background.b - 34.0 / 255.0).abs() < 0.00001);

        let forest = Theme::ForestGreen.palette(Strength::Balanced);
        assert!((forest.background.r - 25.0 / 255.0).abs() < 0.00001);
        assert!((forest.background.g - 35.0 / 255.0).abs() < 0.00001);
        assert!((forest.background.b - 30.0 / 255.0).abs() < 0.00001);
    }

    #[test]
    fn builds_expected_light_transfer_function() {
        let transfer = color_transfer(mocha("text"), mocha("mantle"), SourceMode::Light);
        assert_eq!(
            transfer,
            "{ 1 exch sub 0.709804 mul 0.094118 add } { 1 exch sub 0.745098 mul 0.094118 add } { 1 exch sub 0.811765 mul 0.145098 add } { 1 exch sub 0.709804 mul 0.094118 add } setcolortransfer"
        );
    }

    #[test]
    fn builds_expected_dark_transfer_function() {
        let transfer = color_transfer(mocha("text"), mocha("mantle"), SourceMode::Dark);
        assert_eq!(
            transfer,
            "{ 0.709804 mul 0.094118 add } { 0.745098 mul 0.094118 add } { 0.811765 mul 0.145098 add } { 0.709804 mul 0.094118 add } setcolortransfer"
        );
    }

    #[test]
    fn derives_default_output_path() {
        let output = default_output_path(Path::new("docs/book.pdf"), "_catppuccin_mocha").unwrap();
        assert_eq!(output, PathBuf::from("docs/book_catppuccin_mocha.pdf"));
    }

    #[test]
    fn derives_theme_based_suffix() {
        let palette = Theme::ForestGreen.palette(Strength::Balanced);
        let suffix = format!("_{}", palette.name.replace('-', "_"));
        assert_eq!(suffix, "_forest_green");
    }

    #[test]
    fn treats_extensionless_destination_as_directory() {
        let output =
            output_from_destination(Path::new("docs/book.pdf"), Some(Path::new("out")), "_dark")
                .unwrap();
        assert_eq!(output, PathBuf::from("out/book_dark.pdf"));
    }

    #[test]
    fn treats_pdf_destination_as_file() {
        let output = output_from_destination(
            Path::new("docs/book.pdf"),
            Some(Path::new("out/custom.pdf")),
            "_dark",
        )
        .unwrap();
        assert_eq!(output, PathBuf::from("out/custom.pdf"));
    }
}
