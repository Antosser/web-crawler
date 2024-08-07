# Web Crawler

Finds every page, image, and script on a website (and downloads it)

## Usage

```
Rust Web Crawler

Usage: web-crawler [OPTIONS] <URL>

Arguments:
  <URL>

Options:
  -d, --download
          Download all files
  -c, --crawl-external
          Whether or not to crawl other websites it finds a link to. Might result in downloading the entire internet
  -m, --max-url-length <MAX_URL_LENGTH>
          Maximum url length it allows. Will ignore page it url length reaches this limit [default: 300]
  -e, --exclude <EXCLUDE>
          Will ignore paths that start with these strings (comma-seperated)
      --export <EXPORT>
          Where to export found URLs
      --export-internal <EXPORT_INTERNAL>
          Where to export internal URLs
      --export-external <EXPORT_EXTERNAL>
          Where to export external URLs
  -t, --timeout <TIMEOUT>
          Timeout between requests in milliseconds [default: 100]
  -h, --help
          Print help
  -V, --version
          Print version
```

## How to compile yourself

1. Download Rust
2. Type `cargo build -r`
3. Executable is in `target/release`

**or**

1. Download Rust
2. Install using `cargo install web-crawler`
