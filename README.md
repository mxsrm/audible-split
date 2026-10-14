# audible-split

A lightweight Rust CLI to split Audible `.m4b` audiobooks into individual `.m4a` chapter files using Audible chapter metadata.

Uses FFmpeg for lossless audio extraction and Rayon for parallel processing.

## Requirements

- Rust (2024 edition)
- FFmpeg installed and available in `PATH`

## Installation

```bash
git clone https://github.com/mxsrm/audible-split.git
cd audible-split
cargo build --release
```

## Usage

```bash
audible-split <AUDIOBOOK.m4b> <CHAPTERS.json> [OPTIONS]
```

**Examples:**

```bash
# Split with 4 parallel workers (default)
audible-split book.m4b chapters.json

# Use 8 workers
audible-split book.m4b chapters.json -j 8

# Show version
audible-split --version

# Show help
audible-split --help
```

## Features

- Parallel chapter extraction using Rayon
- AAC stream copying without re-encoding
- Chapter titles and track numbers embedded as metadata
- Automatic filename sanitization
- Supports nested Audible chapter JSON
- Configurable number of workers (`-j`, `--workers`)

## Output

Files are written to a directory named after the input audiobook:

```text
book/
├── 001 - Introduction.m4a
├── 002 - Chapter 1.m4a
└── 003 - Chapter 2.m4a
```

## Notes

Chapter boundaries may differ slightly from the supplied timestamps due to AAC frame alignment. The input audiobook must already be decrypted.

## License

MIT
