# pdfnight

`pdfnight` converts a PDF to a dark-mode palette from the command line. Catppuccin Mocha is the default.

The converter is intentionally conservative about quality: it uses Ghostscript's `pdfwrite` device with image downsampling disabled and lossless Flate image filters. It does not rasterize pages first. Output files can be much larger than the originals.

This still rewrites the PDF through Ghostscript, so the output is not byte-for-byte identical and some unusual PDF internals can change. The goal is no visible resolution loss, not archival preservation of every original object.

By default, `pdfnight` samples the first few pages to detect whether the source PDF is light-mode or already dark-mode. Light PDFs are inverted into the selected theme; dark PDFs keep their dark background dark and remap light foreground content toward the theme foreground.

The default theme is Catppuccin Mocha. Additional presets are based on the browser converter at `https://chizkiyahu.github.io/pdf-dark-mode-converter/`: Claude Warm, Sepia Dark, Midnight Blue, and Forest Green.

## Requirements

- Rust toolchain with `cargo`
- Ghostscript available on `PATH` as `gs`

If Ghostscript is installed somewhere else, pass its path with `--gs`.

## Install

From GitHub:

```sh
cargo install --git https://github.com/bluwine/pdfnight
```

From a specific release tag:

```sh
cargo install --git https://github.com/bluwine/pdfnight --tag v0.1.0
```

## Build

```sh
cargo build --release
```

The binary will be at:

```sh
target/release/pdfnight
```

## Usage

```sh
pdfnight [OPTIONS] <SOURCE_FILE> [DESTINATION_LOCATION]
```

Examples:

```sh
pdfnight book.pdf
pdfnight book.pdf converted/
pdfnight book.pdf converted/book-dark.pdf
pdfnight --theme forest-green book.pdf
pdfnight --theme midnight-blue book.pdf
pdfnight --source-mode dark already-dark.pdf
pdfnight --background crust --strength high-contrast book.pdf
pdfnight --gs /path/to/gs book.pdf
```

If `DESTINATION_LOCATION` is omitted, the converted file is written beside the source file using a suffix based on the selected theme, such as `_catppuccin_mocha` or `_forest_green`.

If `DESTINATION_LOCATION` is an existing directory, ends with a path separator, or has no file extension, it is treated as a directory. If it has an extension, it is treated as the exact output file path.

Useful options:

- `--suffix <TEXT>` changes the default theme-based suffix.
- `--overwrite` replaces existing output PDFs.
- `--dry-run` prints planned work without writing files.
- `--gs <COMMAND>` uses a specific Ghostscript command or path.
- `--theme mocha|claude-warm|sepia-dark|midnight-blue|forest-green` chooses a color preset.
- `--strength soft|balanced|high-contrast` changes the foreground tone.
- `--background base|mantle|crust` changes the page background.
- `--foreground <HEX>` overrides the foreground color.
- `--source-mode auto|light|dark` controls source detection. Default is `auto`.

## Notes

The default palette maps white/light source content toward Catppuccin Mocha `mantle` and black/dark source content toward Mocha `text`. That keeps text readable while making page backgrounds dark. Other themes use the background colors from the referenced browser converter and white foreground text.
