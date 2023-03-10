use clap::Parser;
use colored::Colorize;
use log::{error, info, warn};
use std::{
    fs,
    io::{self},
    path::Path,
};
use swing::Logger;
use url::Url;

/// Rust Web Crawler
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // /// Url of the website you want to crawl
    // #[arg(short, long)]
    url: String,

    /// Download all files
    #[arg(short, long)]
    download: bool,

    /// Whether or not to crawl other websites it finds a link to. Might result in downloading the entire internet
    #[arg(short, long)]
    crawl_external: bool,

    /// Maximum url length it allows. Will ignore page it url length reaches this limit
    #[arg(short, long, default_value_t = 300)]
    max_url_length: u32,

    /// Will ignore paths that start with these strings (comma-seperated)
    #[arg(short, long, use_value_delimiter = true, value_delimiter = ',')]
    exclude: Vec<String>,
}

fn crawl(url: &Url, urls: &mut Vec<Url>, args: &Args) {
    if !urls.iter().any(|x| x.as_str() == url.as_str()) {
        urls.push(url.clone());
    }
    if url.to_string().len() > args.max_url_length as usize {
        return;
    }

    info!("Fetching url: {}", url.to_string());
    let response = reqwest::blocking::get(url.as_str());
    if response.is_err() {
        warn!("Request failed: {}", url.to_string());
        return;
    }
    let content_type = response.as_ref().unwrap().headers().get("content-type");
    if content_type.is_none() {
        warn!(
            "Response header doesn't have content-type: {}",
            url.to_string()
        );
        return;
    }
    let is_html = content_type
        .unwrap()
        .to_str()
        .unwrap()
        .split(";")
        .nth(0)
        .unwrap()
        .to_string()
        == "text/html";
    let response = response.unwrap().text().unwrap();

    'download: {
        if args.download {
            info!("Downloading file...");
            let mut location = std::env::current_dir().unwrap();
            location.push(url.domain().unwrap());
            {
                let mut path = url.path().strip_prefix("/").unwrap_or(url.path());
                path = path.strip_suffix("/").unwrap_or(path);
                path = path.strip_suffix("\\").unwrap_or(path);
                info!("Working directory: {}", location.to_str().unwrap());
                location.push(path);
            }

            if is_html && !location.ends_with(".html") {
                location.push("index.html");
            }
            info!("Location before: {}", location.to_str().unwrap());
            let mut location_without_last_dir = location.clone();
            assert!(location_without_last_dir.pop());
            info!(
                "Creating directories: {}",
                location_without_last_dir.to_str().unwrap()
            );
            match fs::create_dir_all(&location_without_last_dir) {
                Err(e) => {
                    warn!(
                        "Cannot create directory: {}: {}",
                        &location_without_last_dir.to_str().unwrap(),
                        e
                    );
                    break 'download;
                }
                _ => {}
            };

            {
                let mut path = location.to_str().unwrap();
                path = path.strip_suffix("/").unwrap_or(path);
                path = path.strip_suffix("\\").unwrap_or(path);

                if Path::new(path).exists() {
                    warn!("File already exists: {}", path);
                    break 'download;
                }
                info!("Writing to file: {}", path);
                let mut f = fs::File::create(path).unwrap_or_else(|e| {
                    error!("Cannot create file: {}: {}", path, e);
                    panic!();
                });

                io::copy(
                    &mut reqwest::blocking::get(url.to_string()).unwrap(),
                    &mut f,
                )
                .unwrap();
            }
        }
    }

    let mut found: Vec<Url> = vec![];

    if !is_html {
        return;
    }
    info!("Parsing html...");
    let dom = tl::parse(&response, tl::ParserOptions::default());
    if dom.is_err() {
        warn!("Couldn't parse html.");
        return;
    }
    let dom = dom.unwrap();

    info!("Looping over all elements...");
    for element in dom.nodes().iter() {
        let tag = element.as_tag();
        if tag.is_none() {
            continue;
        }
        let tag = tag.unwrap();

        let mut value = tag.attributes().get("href");
        if value.is_none() {
            value = tag.attributes().get("src");
            if value.is_none() {
                continue;
            }
        }
        let value = value.unwrap();
        if value.is_none() {
            continue;
        }
        let value = value.unwrap();
        info!("Found link: {}", value.as_utf8_str().to_string());

        let url = url.join(&value.as_utf8_str().to_string());
        if url.is_err() {
            warn!("Invalid url: {}", value.as_utf8_str().to_string());
            continue;
        }
        info!("Valid: {}", value.as_utf8_str().to_string());
        let url = url.unwrap();

        found.push(url);
    }

    for mut i in found {
        i = Url::parse(i.to_string().split('?').nth(0).unwrap_or(&i.to_string())).unwrap();
        i = Url::parse(i.to_string().split('#').nth(0).unwrap_or(&i.to_string())).unwrap();

        if !urls.iter().any(|x| x.as_str() == i.as_str()) {
            if !args.exclude.iter().any(|j| i.path().starts_with(j)) {
                urls.push(i.clone());
                if url.domain() == i.domain() || args.crawl_external {
                    info!("Url is internal. Crawling: {}", i.to_string());
                    crawl(&i, urls, args);
                }
            }
        }
    }
}

fn main() {
    Logger::new().init().unwrap();

    info!("Parsing arguments...");
    let args = Args::parse();
    info!("{:?}", args);

    let mut found_urls: Vec<Url> = vec![];
    info!("Parsing url...");
    let document = Url::parse(&args.url).unwrap_or_else(|_| {
        error!("Cannot parse url: {}", args.url);
        panic!();
    });

    info!("Crawling...");
    crawl(&document, &mut found_urls, &args);

    let mut internal_urls = Vec::new();
    let mut external_urls = Vec::new();

    for url in found_urls {
        if url.domain() == document.domain() {
            internal_urls.push(url);
        } else {
            external_urls.push(url);
        }
    }

    println!("{}", format!("Internal urls:").red());
    for url in internal_urls {
        println!("{}", url.as_str());
    }

    println!("{}", format!("External urls:").red());
    for url in external_urls {
        println!("{}", url.as_str());
    }
}
