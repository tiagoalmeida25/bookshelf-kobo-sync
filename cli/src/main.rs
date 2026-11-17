use std::collections::{HashMap, HashSet};

use argh::FromArgs;
use reqwest::Client;

mod api;
mod config;
mod sqlite;
mod util;

use crate::config::{CONFIG, SyncBookmarks};
use crate::sqlite::{open_db, read_bookmarks, read_books};

#[derive(FromArgs, Debug)]
/// Bookshelf sync for Kobo. Reads `KoboReader.sqlite` and pushes progress,
/// highlights, ratings and reviews to your Bookshelf library.
struct Args {
  #[argh(subcommand)]
  cmd: Cmd,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum Cmd {
  Sync(SyncCmd),
  Search(SearchCmd),
  Link(LinkCmd),
  CreateBook(CreateBookCmd),
  Ping(PingCmd),
}

/// Read KoboReader.sqlite and POST a full batch to Bookshelf.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "sync")]
struct SyncCmd {
  /// only print what would be uploaded; don't send anything.
  #[argh(switch)]
  dry_run: bool,
}

/// Search the Bookshelf library (and optionally ISBNdb) for a book.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "search")]
struct SearchCmd {
  /// query string - title, author, or ISBN.
  #[argh(positional)]
  query: String,

  /// also query ISBNdb for books not yet in the library.
  #[argh(switch)]
  include_external: bool,
}

/// Persist a manual Kobo→Bookshelf book mapping.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "link")]
struct LinkCmd {
  /// kobo content id (e.g. `file:///mnt/onboard/My Book.epub`)
  #[argh(option)]
  kobo_content_id: String,

  /// bookshelf book id to link this Kobo book to
  #[argh(option)]
  bookshelf_book_id: String,
}

/// Create a new book in the Bookshelf library from a Kobo book.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "create-book")]
struct CreateBookCmd {
  /// kobo content id of the unlinked book
  #[argh(option)]
  kobo_content_id: String,

  /// book title
  #[argh(option)]
  title: String,

  /// book author
  #[argh(option, default = "String::new()")]
  author: String,

  /// isbn (10 or 13 digit), if known
  #[argh(option)]
  isbn: Option<String>,

  /// total pages, if known
  #[argh(option)]
  total_pages: Option<i64>,

  /// cover image url, if known
  #[argh(option)]
  cover_url: Option<String>,
}

/// Smoke-test that the configured token + server URL work.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ping")]
struct PingCmd {}

#[tokio::main]
async fn main() {
  let args: Args = argh::from_env();

  if let Err(e) = run(args).await {
    eprintln!("error: {e}");
    std::process::exit(1);
  }
}

async fn run(args: Args) -> Result<(), String> {
  if CONFIG.authorization.is_empty() || CONFIG.authorization == "REPLACE_WITH_YOUR_TOKEN" {
    return Err(
      "No Bookshelf sync token configured. Open the Bookshelf app, go to Settings → Kobo sync, \
       generate a token, then in .adds/BookshelfSync/config.ini set `authorization = bks_...` \
       (no leading `;`, no surrounding quotes)."
        .into(),
    );
  }

  let client = Client::builder()
    .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
    .build()
    .map_err(|e| format!("build http client failed: {e}"))?;

  match args.cmd {
    Cmd::Sync(s) => run_sync(&client, s).await,
    Cmd::Search(s) => {
      let res = api::search_book(&client, &s.query, s.include_external).await?;
      println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
      Ok(())
    }
    Cmd::Link(l) => {
      let res = api::link_book(&client, &l.kobo_content_id, &l.bookshelf_book_id).await?;
      println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
      Ok(())
    }
    Cmd::CreateBook(c) => {
      let res = api::create_book(
        &client,
        &c.kobo_content_id,
        &c.title,
        &c.author,
        c.isbn.as_deref(),
        c.total_pages,
        c.cover_url.as_deref(),
      )
      .await?;
      println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
      Ok(())
    }
    Cmd::Ping(_) => {
      // Use an empty search as a no-op auth check.
      let res = api::search_book(&client, "__ping__", false).await?;
      println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
      Ok(())
    }
  }
}

async fn run_sync(client: &Client, args: SyncCmd) -> Result<(), String> {
  let conn = open_db()?;
  let books = read_books(&conn)?;

  // Skip books Kobo has never opened - these are usually on-device but
  // never started, and pushing a 0% entry is noise.
  let books: Vec<_> = books
    .into_iter()
    .filter(|b| b.percent_read.unwrap_or(0.0) > 0.0 || b.rating.is_some() || b.review.is_some())
    .collect();

  let bookmarks = match CONFIG.sync_bookmarks {
    SyncBookmarks::Never => Vec::new(),
    SyncBookmarks::Always => read_bookmarks(&conn, None)?,
    SyncBookmarks::Finished => {
      let all = read_bookmarks(&conn, None)?;
      let finished_ids: std::collections::HashSet<&str> = books
        .iter()
        .filter(|b| b.read_status == Some(2))
        .map(|b| b.kobo_content_id.as_str())
        .collect();
      all
        .into_iter()
        .filter(|m| finished_ids.contains(m.kobo_content_id.as_str()))
        .collect()
    }
  };

  util::log(&format!(
    "kobo sqlite scan: {} books, {} bookmarks",
    books.len(),
    bookmarks.len()
  ));

  if args.dry_run {
    println!(
      "{}",
      serde_json::to_string_pretty(&serde_json::json!({
        "books": books,
        "bookmarks": bookmarks,
      }))
      .unwrap_or_default()
    );
    return Ok(());
  }

  let tokens = CONFIG.account_tokens();
  if tokens.is_empty() {
    return Err("No sync token configured. Set `authorization` in config.ini.".into());
  }

  // Single account: unchanged behaviour, and no extra round trips.
  if tokens.len() == 1 {
    let resp = api::send_batch(client, &tokens[0], &books, &bookmarks).await?;
    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_default());
    return Ok(());
  }

  // Shared device. Make sure each account knows about the other so the
  // app can label the per-book switch with a name rather than a uid. Best
  // effort: a failure here only costs the label, and must not stop a sync.
  if let Err(e) = api::register_device_peers(client, &tokens[0], &tokens[1..]).await {
    util::log(&format!("peer registration skipped: {e}"));
  }

  let mut routings = Vec::new();
  for token in &tokens {
    match api::fetch_routing(client, token).await {
      Ok(r) => routings.push(r),
      Err(e) => {
        // Without every account's table the merge below would silently
        // reroute books to the default account, which writes one
        // reader's progress into the other's library. Refuse instead.
        return Err(format!("could not read routing for one account: {e}"));
      }
    }
  }

  let assignment = resolve_routing(&books, &routings, CONFIG.default_account_index());

  let mut responses = Vec::new();
  for (i, token) in tokens.iter().enumerate() {
    let ids: HashSet<&str> = assignment
      .iter()
      .filter(|(_, accounts)| accounts.contains(&i))
      .map(|(id, _)| id.as_str())
      .collect();

    if ids.is_empty() {
      continue;
    }

    let account_books: Vec<_> = books
      .iter()
      .filter(|b| ids.contains(b.kobo_content_id.as_str()))
      .cloned()
      .collect();
    let account_bookmarks: Vec<_> = bookmarks
      .iter()
      .filter(|m| ids.contains(m.kobo_content_id.as_str()))
      .cloned()
      .collect();

    util::log(&format!(
      "account {}: {} books, {} bookmarks",
      i + 1,
      account_books.len(),
      account_bookmarks.len()
    ));

    let resp = api::send_batch(client, token, &account_books, &account_bookmarks).await?;
    responses.push(resp);
  }

  println!(
    "{}",
    serde_json::to_string_pretty(&responses).unwrap_or_default()
  );
  Ok(())
}

/// Decide which configured accounts each book should be pushed to.
///
/// `routings[i]` is what account `i` said, so `routings[i].routing[id]` is
/// the uid that account assigned that book to. The rules, in order:
///
///   * anybody routed it to a *configured* account -> those accounts
///   * both readers claimed it -> both, because two people genuinely
///     reading the same copy is the case this feature exists for, and
///     dropping one of them loses that reader's progress outright
///   * nobody mentioned it -> `default_index`
///
/// A target uid that is not one of the accounts on this device is ignored:
/// it means the reader unpaired, or pointed at an account whose token was
/// removed from config.ini, and the device has no way to write there.
fn resolve_routing(
  books: &[sqlite::KoboBook],
  routings: &[api::RoutingResponse],
  default_index: usize,
) -> HashMap<String, HashSet<usize>> {
  let uid_to_index: HashMap<&str, usize> = routings
    .iter()
    .enumerate()
    .map(|(i, r)| (r.uid.as_str(), i))
    .collect();

  let mut out: HashMap<String, HashSet<usize>> = HashMap::new();
  for book in books {
    let id = &book.kobo_content_id;
    let mut accounts = HashSet::new();
    for routing in routings {
      if let Some(target) = routing.routing.get(id) {
        if let Some(&idx) = uid_to_index.get(target.as_str()) {
          accounts.insert(idx);
        }
      }
    }
    if accounts.is_empty() {
      accounts.insert(default_index);
    }
    out.insert(id.clone(), accounts);
  }
  out
}
