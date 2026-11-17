use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::CONFIG;
use crate::sqlite::{KoboBook, KoboBookmark};

/// One book progress update for `koboSyncBatch`. Field names match the
/// server-side handler in `firebase/functions/kobo_sync.js`.
#[derive(Serialize)]
struct BookProgressPayload<'a> {
  #[serde(rename = "koboContentId")]
  kobo_content_id: &'a str,
  percent: f64,
  #[serde(rename = "timeSpent")]
  time_spent: i64,
  #[serde(rename = "lastReadAt")]
  last_read_at: Option<i64>,
  #[serde(rename = "conflictResolution")]
  conflict_resolution: &'static str,
  isbn: Option<&'a str>,
  // The server captures these for the `kobo_unmatched` Firestore
  // subcollection so the Bookshelf app can render an "Unmatched" tab
  // with recognizable book info (without these the user just sees an
  // opaque content id).
  #[serde(skip_serializing_if = "Option::is_none")]
  title: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  author: Option<&'a str>,
}

#[derive(Serialize)]
struct BookmarkPayload<'a> {
  #[serde(rename = "koboBookmarkId")]
  kobo_bookmark_id: &'a str,
  #[serde(rename = "koboContentId")]
  kobo_content_id: &'a str,
  text: &'a str,
  annotation: Option<&'a str>,
  #[serde(rename = "chapterProgress")]
  chapter_progress: Option<f64>,
  #[serde(rename = "createdAt")]
  created_at: Option<i64>,
}

#[derive(Serialize)]
struct RatingPayload<'a> {
  #[serde(rename = "koboContentId")]
  kobo_content_id: &'a str,
  rating: f64,
}

#[derive(Serialize)]
struct ReviewPayload<'a> {
  #[serde(rename = "koboContentId")]
  kobo_content_id: &'a str,
  review: &'a str,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SyncBatchResponse {
  pub ok: bool,
  #[serde(default, rename = "logId")]
  pub log_id: Option<String>,
  pub applied: serde_json::Value,
  #[serde(default)]
  pub unlinked: Vec<serde_json::Value>,
  #[serde(default, rename = "perBook")]
  pub per_book: Vec<serde_json::Value>,
}

fn iso_to_millis(iso: &str) -> Option<i64> {
  DateTime::parse_from_rfc3339(iso)
    .ok()
    .map(|d| d.with_timezone(&Utc).timestamp_millis())
}

/// Build the JSON body from raw sqlite rows and POST it to `koboSyncBatch`.
/// `include_bookmarks` is decided by the caller based on `sync_bookmarks`
/// config + whether the book is finished.
/// Push a batch to one account.
///
/// [`token`] is explicit rather than read from `CONFIG.authorization`: a
/// Kobo shared by two readers pushes one batch per account, each with that
/// account's own bearer token. That is what keeps the server side
/// unchanged - every write still lands under the uid the token belongs to,
/// so one reader can never write into the other's library.
pub async fn send_batch(
  client: &Client,
  token: &str,
  books: &[KoboBook],
  bookmarks: &[KoboBookmark],
) -> Result<SyncBatchResponse, String> {
  let books_payload: Vec<BookProgressPayload> = books
    .iter()
    .map(|b| BookProgressPayload {
      kobo_content_id: &b.kobo_content_id,
      percent: b.percent_read.unwrap_or(0.0),
      time_spent: b.time_spent_reading.unwrap_or(0),
      last_read_at: b.date_last_read.as_deref().and_then(iso_to_millis),
      conflict_resolution: CONFIG.conflict_resolution.as_str(),
      isbn: b.isbn.as_deref(),
      title: b.title.as_deref(),
      author: b.author.as_deref(),
    })
    .collect();

  let bookmarks_payload: Vec<BookmarkPayload> = bookmarks
    .iter()
    .map(|m| BookmarkPayload {
      kobo_bookmark_id: &m.kobo_bookmark_id,
      kobo_content_id: &m.kobo_content_id,
      text: &m.text,
      annotation: m.annotation.as_deref(),
      chapter_progress: m.chapter_progress,
      created_at: m.created_at.as_deref().and_then(iso_to_millis),
    })
    .collect();

  let ratings_payload: Vec<RatingPayload> = books
    .iter()
    .filter_map(|b| {
      b.rating.map(|r| RatingPayload {
        kobo_content_id: &b.kobo_content_id,
        rating: r as f64,
      })
    })
    .collect();

  let reviews_payload: Vec<ReviewPayload> = books
    .iter()
    .filter_map(|b| {
      b.review.as_deref().map(|r| ReviewPayload {
        kobo_content_id: &b.kobo_content_id,
        review: r,
      })
    })
    .collect();

  let body = json!({
    "books": books_payload,
    "bookmarks": bookmarks_payload,
    "ratings": ratings_payload,
    "reviews": reviews_payload,
    "syncedAt": Utc::now().timestamp_millis(),
  });

  let url = format!("{}/koboSyncBatch", CONFIG.server_url.trim_end_matches('/'));
  let res = client
    .post(&url)
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("POST {url} failed: {e}"))?;

  let status = res.status();
  let text = res
    .text()
    .await
    .map_err(|e| format!("read response body failed: {e}"))?;

  if !status.is_success() {
    return Err(format!("koboSyncBatch returned {status}: {text}"));
  }

  serde_json::from_str(&text).map_err(|e| format!("decode batch response failed: {e}; body={text}"))
}

pub async fn search_book(client: &Client, q: &str, include_external: bool) -> Result<serde_json::Value, String> {
  let url = format!(
    "{}/koboSearchBook?q={}&includeExternal={}",
    CONFIG.server_url.trim_end_matches('/'),
    urlencode(q),
    if include_external { "1" } else { "0" }
  );
  let res = client
    .get(&url)
    .bearer_auth(&CONFIG.authorization)
    .send()
    .await
    .map_err(|e| format!("GET {url} failed: {e}"))?;
  let status = res.status();
  let text = res.text().await.map_err(|e| format!("read body failed: {e}"))?;
  if !status.is_success() {
    return Err(format!("koboSearchBook returned {status}: {text}"));
  }
  serde_json::from_str(&text).map_err(|e| format!("decode search response failed: {e}; body={text}"))
}

pub async fn link_book(
  client: &Client,
  kobo_content_id: &str,
  bookshelf_book_id: &str,
) -> Result<serde_json::Value, String> {
  let url = format!("{}/koboLinkBook", CONFIG.server_url.trim_end_matches('/'));
  let body = json!({
    "koboContentId": kobo_content_id,
    "bookshelfBookId": bookshelf_book_id,
  });
  let res = client
    .post(&url)
    .bearer_auth(&CONFIG.authorization)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("POST {url} failed: {e}"))?;
  let status = res.status();
  let text = res.text().await.map_err(|e| format!("read body failed: {e}"))?;
  if !status.is_success() {
    return Err(format!("koboLinkBook returned {status}: {text}"));
  }
  serde_json::from_str(&text).map_err(|e| format!("decode link response failed: {e}; body={text}"))
}

pub async fn create_book(
  client: &Client,
  kobo_content_id: &str,
  title: &str,
  author: &str,
  isbn: Option<&str>,
  total_pages: Option<i64>,
  cover_url: Option<&str>,
) -> Result<serde_json::Value, String> {
  let url = format!("{}/koboCreateBook", CONFIG.server_url.trim_end_matches('/'));
  let body = json!({
    "koboContentId": kobo_content_id,
    "title": title,
    "author": author,
    "isbn": isbn,
    "totalPages": total_pages,
    "coverUrl": cover_url,
  });
  let res = client
    .post(&url)
    .bearer_auth(&CONFIG.authorization)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("POST {url} failed: {e}"))?;
  let status = res.status();
  let text = res.text().await.map_err(|e| format!("read body failed: {e}"))?;
  if !status.is_success() {
    return Err(format!("koboCreateBook returned {status}: {text}"));
  }
  serde_json::from_str(&text).map_err(|e| format!("decode create response failed: {e}; body={text}"))
}

/// Minimal URL-encoder - we never receive characters outside the
/// alphanumeric + a few punctuation chars in practice, so the table is
/// kept tiny instead of pulling in a dependency.
/// One account's routing table, as returned by `GET /koboSyncRouting`.
#[derive(Deserialize, Debug, Default)]
pub struct RoutingResponse {
  #[serde(default)]
  pub uid: String,
  /// koboContentId → uid the reader assigned that book to.
  #[serde(default)]
  pub routing: HashMap<String, String>,
  #[serde(default)]
  pub peers: Vec<Peer>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Peer {
  #[serde(default)]
  pub uid: String,
  #[serde(default)]
  pub username: String,
}

pub async fn fetch_routing(client: &Client, token: &str) -> Result<RoutingResponse, String> {
  let url = format!("{}/koboSyncRouting", CONFIG.server_url.trim_end_matches('/'));
  let res = client
    .get(&url)
    .bearer_auth(token)
    .send()
    .await
    .map_err(|e| format!("GET {url} failed: {e}"))?;

  let status = res.status();
  let text = res
    .text()
    .await
    .map_err(|e| format!("read response body failed: {e}"))?;

  if !status.is_success() {
    return Err(format!("koboSyncRouting returned {status}: {text}"));
  }

  serde_json::from_str(&text)
    .map_err(|e| format!("decode routing response failed: {e}; body={text}"))
}

/// Tell the accounts on this device about each other, so each app can show
/// the other's name next to the per-book switch.
///
/// Authenticated with the first token and carrying the rest in the body:
/// the server verifies every one of them before writing anything, so this
/// cannot be used to attach yourself to a library you don't hold a token
/// for.
pub async fn register_device_peers(
  client: &Client,
  token: &str,
  peer_tokens: &[String],
) -> Result<(), String> {
  let url = format!(
    "{}/koboRegisterDevicePeers",
    CONFIG.server_url.trim_end_matches('/')
  );
  let res = client
    .post(&url)
    .bearer_auth(token)
    .json(&json!({ "peerTokens": peer_tokens }))
    .send()
    .await
    .map_err(|e| format!("POST {url} failed: {e}"))?;

  let status = res.status();
  if !status.is_success() {
    let text = res.text().await.unwrap_or_default();
    return Err(format!("koboRegisterDevicePeers returned {status}: {text}"));
  }
  Ok(())
}

fn urlencode(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for b in s.as_bytes() {
    match *b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
      _ => out.push_str(&format!("%{:02X}", b)),
    }
  }
  out
}
