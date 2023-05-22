use clap::Parser;
use colored::Colorize;
use log::{debug, error, info, trace, warn};
use std::time;
use std::{
    borrow::Borrow,
    fs,
    io::Write,
    path::Path,
    process::exit,
    sync::{Arc, Mutex},
    thread,
};
use url::Url;

/// Rust Web Crawler
#[derive(Parser, Debug, Clone)]
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

    /// Where to export found URLs
    #[arg(long)]
    export: Option<String>,

    /// Where to export internal URLs
    #[arg(long)]
    export_internal: Option<String>,

    /// Where to export external URLs
    #[arg(long)]
    export_external: Option<String>,

    /// Timeout between requests in milliseconds
    #[arg(short, long, default_value_t = 100)]
    timeout: u64,
}

fn crawl(
    url: &Url,
    urls: Arc<Mutex<Vec<Url>>>,
    args: Arc<Args>,
    latest_request: Arc<Mutex<time::Instant>>,
) {
    {
        let mut urls = urls.lock().unwrap();

        if !urls.iter().any(|x| x.as_str() == url.as_str()) {
            urls.push(url.clone());
        }
        if url.to_string().len() > args.max_url_length as usize {
            warn!("URL too long: {}", url);
            return;
        }
    }

    // Wait for timeout
    {
        let mut latest_request = latest_request.lock().unwrap();
        let time_since_last_request = latest_request.elapsed();
        if time_since_last_request < time::Duration::from_millis(args.timeout) {
            thread::sleep(time::Duration::from_millis({
                let time = args.timeout - time_since_last_request.as_millis() as u64;
                debug!("Sleeping for {}ms", time);
                time
            }));
        }

        *latest_request = time::Instant::now();
    }
    trace!("Fetching url: {}", url.to_string());
    let response = match reqwest::blocking::get(url.as_str()) {
        Ok(x) => x,
        Err(e) => {
            error!("Cannot request file: {}", e);
            return;
        }
    };
    let is_html = match response.headers().get("content-type") {
        Some(content_type) => match content_type.to_str() {
            Ok(content_type_string) => match content_type_string.split(';').next() {
                Some(x) => x == "text/html",
                None => {
                    warn!("Cannot get content-type: {}", url);
                    false
                }
            },
            Err(_) => {
                warn!("Cannot get content-type: {}", url);
                false
            }
        },
        None => {
            warn!(
                "Response header doesn't have content-type: {}",
                url.to_string()
            );
            false
        }
    };
    let response_bytes = match response.bytes() {
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
                let mut relative_path = url.path().strip_prefix('/').unwrap_or(url.path());
                relative_path = relative_path.strip_suffix('/').unwrap_or(relative_path);
                relative_path = relative_path.strip_suffix('\\').unwrap_or(relative_path);
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
            if let Err(e) = fs::create_dir_all(&path_without_last_dir) {
                warn!(
                    "Cannot create directory: {}: {}",
                    path_without_last_dir_string, e
                );
                break 'download;
            }
            {
                let mut file_path = path_string;
                file_path = file_path.strip_suffix('/').unwrap_or(file_path);
                file_path = file_path.strip_suffix('\\').unwrap_or(file_path);

                if Path::new(file_path).exists() {
                    warn!("File already exists: {}", file_path);
                    break 'download;
                }
                trace!("Writing to file: {}", file_path);
                let mut f = match fs::File::create(file_path) {
                    Ok(x) => x,
                    Err(e) => {
                        error!("Cannot create file: {}: {}", file_path, e);
                        return;
                    }
                };

                match f.write_all(&response_bytes) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Cannot write to file: {}: {}", file_path, e);
                        return;
                    }
                };
            }
        }
    }

    let mut found: Vec<Url> = vec![];

    if !is_html {
        return;
    }
    let response_text = String::from_utf8_lossy(&response_bytes);
    debug!("Parsing html...");
    let dom = match tl::parse(&response_text, tl::ParserOptions::default()) {
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

        let value = match {
            match tag.attributes().get("href") {
                Some(x) => x,
                None => match tag.attributes().get("src") {
                    Some(x) => x,
                    None => continue,
                },
            }
        } {
            Some(x) => x,
            None => continue,
        };
        trace!("Found link: {}", value.as_utf8_str().to_string());

        let url = match url.join(&value.as_utf8_str()) {
            Ok(x) => x,
            Err(e) => {
                warn!("Cannot join url: {}", e);
                continue;
            }
        };
        trace!("Valid: {}", value.as_utf8_str().to_string());

        found.push(url);
    }

    let mut threads = vec![];

    {
        let mut urls_locked = urls.lock().unwrap();

        for mut i in found {
            i = Url::parse(i.to_string().split('?').next().unwrap_or(i.as_ref())).unwrap(); // Unreachable .unwrap()
            i = Url::parse(i.to_string().split('#').next().unwrap_or(i.as_ref())).unwrap(); // Unreachable .unwrap()

            if !urls_locked.iter().any(|x| x.as_str() == i.as_str())
                && !args.exclude.iter().any(|j| i.path().starts_with(j))
            {
                info!("Found url: {}", i);
                urls_locked.push(i.clone());
                if url.domain() == i.domain() || args.crawl_external {
                    trace!("Url is internal. Crawling: {}", i.to_string());
                    {
                        let urls = urls.clone();
                        let args = args.clone();
                        let latest_request = latest_request.clone();

                        threads.push(thread::spawn(move || {
                            crawl(&i, urls, args, latest_request);
                        }));
                    }
                }
            }
        }
    }

    for thread in threads {
        thread.join().unwrap();
    }
}

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    debug!("Parsing arguments...");
    let args = Args::parse();
    trace!("{:?}", args);

    let found_urls: Arc<Mutex<Vec<Url>>> = Arc::new(Mutex::new(vec![]));
    trace!("Parsing url...");
    let document = Url::parse(&args.url).unwrap_or_else(|_| {
        error!("Cannot parse url: {}", args.url);
        exit(1);
    });

    debug!("Crawling...");
    crawl(
        &document,
        found_urls.clone(),
        Arc::new(args.clone()),
        Arc::new(Mutex::new(time::Instant::now())),
    );

    let mut found_urls = found_urls.lock().unwrap();
    found_urls.sort();

    let mut internal_urls = Vec::new();
    let mut external_urls = Vec::new();

    for url in found_urls.iter() {
        if url.domain() == document.domain() {
            internal_urls.push(url);
        } else {
            external_urls.push(url);
        }
    }

    println!("{}", "Internal urls:".to_string().bright_green());
    for url in &internal_urls {
        println!("{}", url.as_str());
    }

    println!("{}", "External urls:".to_string().red());
    for url in &external_urls {
        println!("{}", url.as_str());
    }

    fn export<T: Borrow<Url>>(file_name: &str, found_urls: &[T]) {
        'export: {
            let mut file = match fs::File::create(file_name) {
                Ok(x) => x,
                Err(e) => {
                    error!("Cannot create file: {}: {}", file_name, e);
                    break 'export;
                }
            };

            for url in found_urls.iter() {
                match file.write_all(format!("{}\n", url.borrow().as_str()).as_bytes()) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Cannot write to file: {}: {}", file_name, e);
                        break 'export;
                    }
                }
            }
        }

        info!("Exported to file: {}", file_name);
    }

    if let Some(file_name) = args.export {
        export(&file_name, &found_urls);
    }
    if let Some(file_name) = args.export_internal {
        export(&file_name, &internal_urls);
    }
    if let Some(file_name) = args.export_external {
        export(&file_name, &external_urls);
    }
}
