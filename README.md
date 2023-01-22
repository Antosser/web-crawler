# Web Crawler
Finds every page, image, and script on a website (and downloads it)

## Usage
```
Usage: web-crawler [OPTIONS] --url <URL>

Options:
  -u, --url <URL>
          Url of the website you want to crawl
  -d, --download
          Download all files
  -c, --crawl-external
          Whether or not to crawl other websites it finds a link to. Might result in downloading the entire internet
  -m, --max-url-length <MAX_URL_LENGTH>
          Maximum url length it allows. Will ignore page it url length reaches this limit [default: 300]
  -h, --help
          Print help
  -V, --version
          Print version
```

## How to compile yourself
1. Download rust
2. Type `cargo build -r`
3. Executable is in `target/release`
