use rusqlite::{Connection, OpenFlags, Row};
use serde::Serialize;

use crate::config::CONFIG;

/// A Kobo book row, as read from `content` where ContentType = 6
/// (top-level book - not a chapter).
#[derive(Debug, Serialize, Clone)]
pub struct KoboBook {
  /// Kobo's stable identifier for the volume (`content.ContentId`). Can be a
  /// `file:///` URI for sideloaded books or an ID like
  /// `00000000-0000-0000-0000-000000000000` for purchased ones.
  pub kobo_content_id: String,
  pub title: Option<String>,
  pub author: Option<String>,
  pub isbn: Option<String>,
  /// 0..100 - Kobo stores this directly. Use `normalised_percent` to
  /// convert to a 0..1 fraction.
  pub percent_read: Option<f64>,
  /// `content.ReadStatus` (0 unread, 1 reading, 2 finished).
  pub read_status: Option<i64>,
  /// `content.TimeSpentReading` in seconds.
  pub time_spent_reading: Option<i64>,
  /// `content.DateLastRead` as ISO8601 - converted to millis at the
  /// payload stage.
  pub date_last_read: Option<String>,
  /// Best effort: ``BookInsertedDate``-ish field. Useful only for ordering.
  pub date_added: Option<String>,
  /// Star rating recorded via the Kobo native rating widget (1..5), or
  /// None if the user hasn't rated.
  pub rating: Option<i64>,
  /// User-entered review text, if any (Kobo lets you write a one-paragraph
  /// "What did you think?" blurb when finishing a book).
  pub review: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct KoboBookmark {
  /// Stable identifier - used to dedupe on Bookshelf's side.
  pub kobo_bookmark_id: String,
  pub kobo_content_id: String,
  pub text: String,
  pub annotation: Option<String>,
  /// 0..1 fraction through the book where the bookmark lives. Bookshelf
  /// turns this into a page number via `totalPages`.
  pub chapter_progress: Option<f64>,
  /// ISO8601. Converted to millis at the payload stage.
  pub created_at: Option<String>,
}

fn opt_string(row: &Row, idx: usize) -> rusqlite::Result<Option<String>> {
  let raw: Option<String> = row.get(idx)?;
  Ok(raw.filter(|s| !s.is_empty()))
}

pub fn open_db() -> Result<Connection, String> {
  Connection::open_with_flags(&CONFIG.sqlite_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    .map_err(|e| format!("Failed to open {}: {e}", &CONFIG.sqlite_path))
}

/// Read every top-level book Kobo knows about. We rely on `ContentType = 6`
/// so we don't return chapter sub-rows. Only columns that are present on
/// every Kobo firmware in active use are queried directly - `Rating` and
/// `Review` live in a `content` column on some firmwares and an `Activity`
/// row on others, so we leave both as `None` here for now and let the user
/// rate/review in the Bookshelf app instead.
pub fn read_books(conn: &Connection) -> Result<Vec<KoboBook>, String> {
  let mut stmt = conn
    .prepare(
      "SELECT
        ContentID,
        Title,
        Attribution,
        ISBN,
        ___PercentRead,
        ReadStatus,
        TimeSpentReading,
        DateLastRead,
        DateCreated
      FROM content
      WHERE ContentType = 6",
    )
    .map_err(|e| format!("prepare books query failed: {e}"))?;

  let rows = stmt
    .query_map([], |row| {
      Ok(KoboBook {
        kobo_content_id: row.get(0)?,
        title: opt_string(row, 1)?,
        author: opt_string(row, 2)?,
        isbn: opt_string(row, 3)?,
        percent_read: row.get(4).ok(),
        read_status: row.get(5).ok(),
        time_spent_reading: row.get(6).ok(),
        date_last_read: opt_string(row, 7)?,
        date_added: opt_string(row, 8)?,
        rating: None,
        review: None,
      })
    })
    .map_err(|e| format!("execute books query failed: {e}"))?;

  let mut out = Vec::new();
  for r in rows {
    out.push(r.map_err(|e| format!("books row decode failed: {e}"))?);
  }
  Ok(out)
}

/// Read all bookmarks (highlights + annotations). `since_iso` is the last
/// successful sync's `DateModified` lower bound - pass `None` to sync
/// everything.
pub fn read_bookmarks(conn: &Connection, since_iso: Option<&str>) -> Result<Vec<KoboBookmark>, String> {
  let default_since = "2000-01-01T00:00:00.000".to_string();
  let since = since_iso.unwrap_or(&default_since);

  let mut stmt = conn
    .prepare(
      "SELECT
        BookmarkID,
        VolumeID,
        Text,
        Annotation,
        ChapterProgress,
        DateCreated
      FROM Bookmark
      WHERE Hidden = 'false'
        AND DateModified > (?1)
        AND Text IS NOT NULL
        AND Text != ''",
    )
    .map_err(|e| format!("prepare bookmarks query failed: {e}"))?;

  let rows = stmt
    .query_map([since], |row| {
      Ok(KoboBookmark {
        kobo_bookmark_id: row.get(0)?,
        kobo_content_id: row.get(1)?,
        text: row.get(2)?,
        annotation: opt_string(row, 3)?,
        chapter_progress: row.get(4).ok(),
        created_at: opt_string(row, 5)?,
      })
    })
    .map_err(|e| format!("execute bookmarks query failed: {e}"))?;

  let mut out = Vec::new();
  for r in rows {
    out.push(r.map_err(|e| format!("bookmarks row decode failed: {e}"))?);
  }
  Ok(out)
}
