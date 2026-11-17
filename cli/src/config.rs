use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Deserializer, Serialize, de};

#[derive(Serialize, PartialEq, Debug, Clone, Copy)]
pub enum SyncBookmarks {
  Always,
  Never,
  Finished,
}

impl<'de> Deserialize<'de> for SyncBookmarks {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    match String::deserialize(deserializer)?.to_ascii_lowercase().as_str() {
      "always" => Ok(SyncBookmarks::Always),
      "never" => Ok(SyncBookmarks::Never),
      "finished" => Ok(SyncBookmarks::Finished),
      s => Err(de::Error::custom(format!("invalid sync_bookmarks `{s}`"))),
    }
  }
}

#[derive(Serialize, PartialEq, Debug, Clone, Copy)]
pub enum ConflictResolution {
  Kobo,
  Bookshelf,
  Auto,
}

impl<'de> Deserialize<'de> for ConflictResolution {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    match String::deserialize(deserializer)?.to_ascii_lowercase().as_str() {
      "kobo" => Ok(ConflictResolution::Kobo),
      "bookshelf" => Ok(ConflictResolution::Bookshelf),
      "auto" => Ok(ConflictResolution::Auto),
      s => Err(de::Error::custom(format!("invalid conflict_resolution `{s}`"))),
    }
  }
}

impl ConflictResolution {
  pub fn as_str(self) -> &'static str {
    match self {
      ConflictResolution::Kobo => "kobo",
      ConflictResolution::Bookshelf => "bookshelf",
      ConflictResolution::Auto => "auto",
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Config {
  /// Bookshelf sync token from the Flutter app's Settings → Kobo sync page.
  pub authorization: String,

  /// A second account's sync token, for a Kobo shared by two readers.
  ///
  /// Empty means single-account, which is the behaviour this mod had
  /// before and still the default. When set, each book is pushed to
  /// whichever account claimed it (see `main::resolve_routing`); the
  /// per-book choice is made in the Bookshelf app, not here.
  ///
  /// Each account needs its own Premium subscription, because generating a
  /// token requires one.
  pub authorization_2: String,

  /// Which account gets a book that neither reader has claimed: 1 for
  /// `authorization`, 2 for `authorization_2`.
  pub default_account: u8,

  /// Base URL of the Bookshelf Cloud Functions endpoint (no trailing slash).
  pub server_url: String,

  /// Path to KoboReader.sqlite. The default works on every Kobo released
  /// since the original Touch.
  pub sqlite_path: String,

  /// What to do when Bookshelf already has more progress for a book than
  /// the Kobo currently reports.
  pub conflict_resolution: ConflictResolution,

  /// When to upload highlights/annotations.
  pub sync_bookmarks: SyncBookmarks,

  /// Enable verbose stderr logging.
  pub debug: bool,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      authorization: String::new(),
      authorization_2: String::new(),
      default_account: 1,
      server_url: "https://us-central1-bookshelf-a829b.cloudfunctions.net".into(),
      sqlite_path: "/mnt/onboard/.kobo/KoboReader.sqlite".into(),
      conflict_resolution: ConflictResolution::Auto,
      sync_bookmarks: SyncBookmarks::Always,
      debug: false,
    }
  }
}

impl Config {
  /// The configured account tokens, in order, with blanks dropped.
  ///
  /// Trimmed because these are pasted into an ini file by hand and a
  /// trailing space in a bearer token produces a 401 that looks like a
  /// revoked token.
  pub fn account_tokens(&self) -> Vec<String> {
    [self.authorization.as_str(), self.authorization_2.as_str()]
      .iter()
      .map(|t| t.trim().to_string())
      .filter(|t| !t.is_empty())
      .collect()
  }

  /// Index into [`Self::account_tokens`] for books nobody has claimed.
  /// Falls back to the first account if `default_account` names one that
  /// is not configured.
  pub fn default_account_index(&self) -> usize {
    let idx = self.default_account.saturating_sub(1) as usize;
    if idx < self.account_tokens().len() { idx } else { 0 }
  }
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
  // Look beside the binary first - on the Kobo this is
  // /mnt/onboard/.adds/BookshelfSync/, matching what `hook/src/files.h`
  // declares and what the web setup guide tells users to create. Fall
  // back to a couple of well-known locations so the CLI is usable from a
  // normal dev workstation too.
  let candidates: Vec<PathBuf> = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|d| d.join("config.ini")))
    .into_iter()
    .chain([
      PathBuf::from("/mnt/onboard/.adds/BookshelfSync/config.ini"),
      PathBuf::from("./config.ini"),
    ])
    .collect();

  let path = candidates.iter().find(|p| p.exists());

  let raw = match path {
    Some(p) => fs::read_to_string(p).unwrap_or_default(),
    None => String::new(),
  };

  serini::from_str::<Config>(&raw).unwrap_or_default()
});
