#include <QJsonObject>
#include <QLabel>

/// One row in the "Manually link book" search dialog. Built from a single
/// element of the `koboSearchBook` response (see firebase/functions/
/// kobo_sync.js - `bookshelfBookId`, `title`, `author`, `isbn`,
/// `coverUrl`, `source`).
class BookRow : public QWidget {
  Q_OBJECT

public:
  BookRow(QJsonObject json, QWidget *parent = nullptr);

public Q_SLOTS:
  void rowTapped();
  void loadCover();

Q_SIGNALS:
  /// Emitted with the bookshelfBookId so the parent dialog can persist
  /// the manual link.
  void tapped(QString bookshelfBookId);

private:
  QString bookshelfBookId;
  QLabel *cover = nullptr;

  QLabel *buildCover(QJsonObject json);
  QWidget *buildTitle(QJsonObject json);
  QWidget *buildAuthor(QJsonObject json);
  QWidget *buildMeta(QJsonObject json);
};
