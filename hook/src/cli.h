#include <QJsonObject>
#include <QLabel>
#include <QObject>
#include <QStringList>

/// Thin Qt wrapper around the `bookshelf-kobo` Rust binary that lives at
/// /mnt/onboard/.adds/BookshelfSync/bookshelf-kobo. Each static factory
/// builds an argv vector matching the CLI's `argh` subcommands and spawns
/// it as a QProcess. The CLI prints a JSON object on stdout, which is
/// emitted via the `response` signal so dialogs can drive their UI off it.
///
/// The CLI's argv shape is intentionally narrower than NickelHardcover's:
/// because the Bookshelf backend ingests the whole library in a single
/// `koboSyncBatch` call we don't need per-book progress mutations.
class CLI : public QObject {
  Q_OBJECT

public:
  /// Run `bookshelf-kobo sync` - reads KoboReader.sqlite and POSTs the
  /// full batch (progress/highlights/ratings/reviews) to `koboSyncBatch`.
  /// `silent` mirrors the Hardcover wifi-prompt suppression: skip the
  /// "connect Wi-Fi" dialog when this is an auto-sync.
  static CLI *sync(bool silent);

  /// Run `bookshelf-kobo search "<q>" --include-external`. The response
  /// JSON is `{ results: [{ bookshelfBookId?, title, author, isbn,
  /// coverUrl, source, totalPages?, ... }] }`. Results with `source ==
  /// 'isbndb'` have no `bookshelfBookId` and require a create-book pass.
  static CLI *search(QString query);

  /// Run `bookshelf-kobo link --kobo-content-id ... --bookshelf-book-id ...`.
  /// Persists the mapping in the `kobo_links` Firestore subcollection so
  /// subsequent syncs skip the ISBN auto-match.
  static CLI *link(QString koboContentId, QString bookshelfBookId);

public Q_SLOTS:
  void networkConnected();
  void connectingFailed();
  void processFinished(int exitCode);

Q_SIGNALS:
  void response(QJsonObject doc);
  void success();
  void failure();

private:
  CLI(QStringList arguments, bool silent = false, QObject *parent = nullptr);

  QLabel *wifiIcon = nullptr;
  QTimer *timer = nullptr;
  QStringList arguments;
  bool silent = false;
};
