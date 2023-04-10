use clap::Parser;
use colored::Colorize;
use log::{debug, error, info, trace, warn};
use std::{
    fs,
    io::{self},
    path::Path,
    process::exit,
};
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

    trace!("Fetching url: {}", url.to_string());
    let response = match reqwest::blocking::get(url.as_str()) {
        Ok(x) => x,
        Err(e) => {
            error!("Cannot request file: {}", e);
            exit(1);
        }
    };
    let content_type = match response.headers().get("content-type") {
        Some(x) => x,
        None => {
            warn!(
                "Response header doesn't have content-type: {}",
                url.to_string()
            );
            return;
        }
    };
    let is_html = match content_type.to_str() {
        Ok(x) => match x.split(";").nth(0) {
            Some(x) => x.to_string() == "text/html",
            None => {
                warn!("Cannot get content-type: {}", url);
                false
            }
        },
        Err(_) => {
            warn!("Cannot get content-type: {}", url);
            false
        }
    };
    let response = match response.text() {
        Ok(x) => x,
        Err(e) => {
            warn!("Cannot parse response as text: {}: {}", url, e);
            return;
        }
    };

    'download: {
        if args.download {
            trace!("Downloading file...");
            let mut path = match std::env::current_dir() {
                Ok(x) => x,
                Err(e) => {
                    error!("Cannot get current working directory: {}", e);
                    exit(1);
                }
            };
            path.push(match url.domain() {
                Some(x) => x,
                None => {
                    warn!("Cannot get domain of url: {}", url);
                    return;
                }
            });

            let path2 = path.clone();
            let path_string = match path2.to_str() {
                Some(x) => x,
                None => {
                    warn!("Couldn't stringify path");
                    return;
                }
            };

            {
                let mut relative_path = url.path().strip_prefix("/").unwrap_or(url.path());
                relative_path = relative_path.strip_suffix("/").unwrap_or(relative_path);
                relative_path = relative_path.strip_suffix("\\").unwrap_or(relative_path);
                trace!("Working directory: {}", path_string);
                path.push(relative_path);
            }

            if is_html && !path.ends_with(".html") {
                path.push("index.html");
            }
            let path2 = path.clone();
            let path_string = match path2.to_str() {
                Some(x) => x,
                None => {
                    warn!("Couldn't stringify path");
                    return;
                }
            };
            trace!("Location before: {}", path_string);
            let mut path_without_last_dir = path.clone();
            assert!(path_without_last_dir.pop());
            let path_without_last_dir_string = match path_without_last_dir.to_str() {
                Some(x) => x,
                None => {
                    warn!("Couldn't stringify path");
                    return;
                }
            };
            trace!("Creating directories: {}", path_without_last_dir_string);
            match fs::create_dir_all(&path_without_last_dir) {
                Err(e) => {
                    warn!(
                        "Cannot create directory: {}: {}",
                        path_without_last_dir_string, e
                    );
                    break 'download;
                }
                _ => {}
            };

            {
                let mut file_path = path_string;
                file_path = file_path.strip_suffix("/").unwrap_or(file_path);
                file_path = file_path.strip_suffix("\\").unwrap_or(file_path);

                if Path::new(file_path).exists() {
                    warn!("File already exists: {}", file_path);
                    break 'download;
                }
                trace!("Writing to file: {}", file_path);
                let mut f = fs::File::create(file_path).unwrap_or_else(|e| {
                    error!("Cannot create file: {}: {}", file_path, e);
                    exit(1);
                });

                match io::copy(&mut response.as_bytes(), &mut f) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Cannot create file: {}: {}", file_path, e);
                        exit(1);
                    }
                };
            }
        }
    }

    let mut found: Vec<Url> = vec![];

    if !is_html {
        return;
    }
    debug!("Parsing html...");
    let dom = match tl::parse(&response, tl::ParserOptions::default()) {
        Ok(x) => x,
        Err(e) => {
            warn!("Cannot parse html: {}: {}", url, e);
            return;
        }
    };

    trace!("Looping over all elements...");
    for element in dom.nodes().iter() {
        let tag = match element.as_tag() {
            Some(x) => x,
            None => {
                continue;
            }
        };

        let value = match match tag.attributes().get("href") {
            Some(x) => x,
            None => match tag.attributes().get("src") {
                Some(x) => x,
                None => continue,
            },
        } {
            Some(x) => x,
            None => continue,
        };
        trace!("Found link: {}", value.as_utf8_str().to_string());

        let url = match url.join(&value.as_utf8_str().to_string()) {
            Ok(x) => x,
            Err(e) => {
                warn!("Cannot join url: {}", e);
                continue;
            }
        };
        trace!("Valid: {}", value.as_utf8_str().to_string());

        found.push(url);
    }

    for mut i in found {
        i = Url::parse(i.to_string().split('?').nth(0).unwrap_or(&i.to_string())).unwrap(); // Unreachable .unwrap()
        i = Url::parse(i.to_string().split('#').nth(0).unwrap_or(&i.to_string())).unwrap(); // Unreachable .unwrap()

        if !urls.iter().any(|x| x.as_str() == i.as_str()) {
            if !args.exclude.iter().any(|j| i.path().starts_with(j)) {
                info!("Found url: {}", i);
                urls.push(i.clone());
                if url.domain() == i.domain() || args.crawl_external {
                    trace!("Url is internal. Crawling: {}", i.to_string());
                    crawl(&i, urls, args);
                }
            }
        }
    }
}

fn main() {
    env_logger::init();

    debug!("Parsing arguments...");
    let args = Args::parse();
    trace!("{:?}", args);

    let mut found_urls: Vec<Url> = vec![];
    trace!("Parsing url...");
    let document = Url::parse(&args.url).unwrap_or_else(|_| {
        error!("Cannot parse url: {}", args.url);
        exit(1);
    });

    debug!("Crawling...");
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
