use scraper::{Html, Selector};
use tokio::sync::mpsc::{channel, Sender};
use url::Url;

const AMAZON: &'static str = "https://www.amazon.com/";
const DOCS_RS: &'static str = "https://docs.rs/";
const MOZILLA: &'static str = "https://www.mozilla.org/";
const RUST_LANG: &'static str = "https://www.rust-lang.org/";
const WIKIPEDIA: &'static str = "http://www.wikipedia.org/";

type MyError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
struct Msg {
    site: Url,
    link: Url,
}

const MSG_BUF_SIZE: usize = 4;

const MAX_DEPTH: usize = 2;

// Use "RUST_LOG=crawler cargo run" to show all traces.
// Use "RUST_LOG=crawler=error cargo run" to show all error traces.

#[tokio::main]
async fn main() -> Result<(), MyError> {
    tracing_subscriber::fmt()
        // line below parses directives from `RUST_LOG` environment variable.
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        // line below makes the tracing output go to stderr.
        .with_writer(std::io::stderr)
        // line below makes this *the* subscriber for all tracing in this app.
        .init();

    let mut sites: Vec<Url> = Vec::new();
    for link in [AMAZON, DOCS_RS, MOZILLA, RUST_LANG, WIKIPEDIA] {
        sites.push(Url::parse(link)?);
    }

    let mut todo: Vec<Url> = sites;
    for _depth in 0..MAX_DEPTH {
        todo = crawl_sites(todo).await?;
    }

    Ok(())
}

async fn crawl_sites(sites: impl IntoIterator<Item = Url>) -> Result<Vec<Url>, MyError> {
    let mut discovered: Vec<Url> = Vec::new();
    let (tx, mut rx) = channel::<Msg>(MSG_BUF_SIZE);

    let mut site_handles = Vec::new();
    for site in sites {
        let site_clone = site.clone();
        let tx = tx.clone();
        let handle = tokio::task::spawn(async {
            let res = all_urls(site_clone, tx).await;
            if res.is_ok() {
                tracing::trace!("crawler result: {:?}", res);
            } else {
                tracing::error!("crawler error: {:?}", res);
            }
            res
        });
        site_handles.push((site, handle));
    }

    drop(tx);

    while let Some(msg) = rx.recv().await {
        println!("site: {} => link: {}", msg.site, msg.link);
        discovered.push(msg.link);
    }

    for (site, handle) in site_handles {
        dbg!((site, handle.await??));
    }

    Ok(discovered)
}

async fn _always_err() -> Result<(), MyError> {
    Err("demo-err".into())
}

async fn all_urls(site: Url, tx: Sender<Msg>) -> Result<usize, MyError> {
    let response: reqwest::Response = reqwest::get(site.clone()).await?;
    let text: String = response.text().await?;
    let urls: Vec<Url> = tokio::task::spawn_blocking(move || {
        all_urls_in_text(&text) // blocking
    })
    .await?
    .unwrap_or_else(|err| panic!("Failed to get URL vector: {:?}.", err));
    let count: usize = urls.len();
    for url in urls {
        tx.send(Msg {
            site: site.clone(),
            link: url,
        })
        .await?
    }
    Ok(count)
}

// Use Tokio spawn_blocking when calling so you don't have to rewrite everything in async.
fn all_urls_in_text(text: &str) -> Result<Vec<Url>, MyError> {
    let mut discovered: Vec<Url> = Vec::new();
    let doc: Html = Html::parse_document(&text);
    // This unwrap should never fail; the input is a known constant.
    let selector: Selector =
        Selector::parse("a").unwrap_or_else(|err| panic!("Failed to parse tag `a`: {:?}.", err));
    for element in doc.select(&selector) {
        let link: &str = match element.value().attr("href") {
            Some(link) => link,
            None => continue,
        };
        let url: Url = match Url::parse(link) {
            Ok(u) => u,
            Err(_) => continue,
        };
        discovered.push(url);
    }
    Ok(discovered)
}
