#include <QHBoxLayout>
#include <QJsonObject>
#include <QNetworkReply>

#include <NickelHook.h>

#include "../files.h"
#include "../synccontroller.h"
#include "../widgets/elidedlabel.h"
#include "bookrow.h"

BookRow::BookRow(QJsonObject json, QWidget *parent)
    : QWidget(parent), bookshelfBookId(json.value("bookshelfBookId").toString()) {
  setStyleSheet(R"(
    [qApp_deviceIsTrilogy=true] SettingContainer {
      qproperty-spacing: 12;
      qproperty-topMargin: 12;
      qproperty-rightMargin: 12;
      qproperty-bottomMargin: 12;
      qproperty-leftMargin: 12;
    }
    [qApp_deviceIsPhoenix=true] SettingContainer {
      qproperty-spacing: 15;
      qproperty-topMargin: 15;
      qproperty-rightMargin: 15;
      qproperty-bottomMargin: 15;
      qproperty-leftMargin: 15;
    }
    [qApp_deviceIsDragon=true] SettingContainer {
      qproperty-spacing: 20;
      qproperty-topMargin: 20;
      qproperty-rightMargin: 20;
      qproperty-bottomMargin: 20;
      qproperty-leftMargin: 20;
    }
    [qApp_deviceIsStorm=true] SettingContainer {
      qproperty-spacing: 22;
      qproperty-topMargin: 22;
      qproperty-rightMargin: 22;
      qproperty-bottomMargin: 22;
      qproperty-leftMargin: 22;
    }
    [qApp_deviceIsDaylight=true] SettingContainer {
      qproperty-spacing: 26;
      qproperty-topMargin: 26;
      qproperty-rightMargin: 26;
      qproperty-bottomMargin: 26;
      qproperty-leftMargin: 26;
    }

    [qApp_deviceIsTrilogy=true] QLabel#blank-cover,
    [qApp_deviceIsTrilogy=true] QLabel#cover {
      max-width: 60px;
      min-width: 60px;
      max-height: 90px;
      min-height: 90px;
    }
    [qApp_deviceIsPhoenix=true] QLabel#blank-cover,
    [qApp_deviceIsPhoenix=true] QLabel#cover {
      max-width: 70px;
      min-width: 70px;
      max-height: 110px;
      min-height: 110px;
    }
    [qApp_deviceIsDragon=true] QLabel#blank-cover,
    [qApp_deviceIsDragon=true] QLabel#cover {
      max-width: 108px;
      min-width: 108px;
      max-height: 168px;
      min-height: 168px;
    }
    [qApp_deviceIsStorm=true] QLabel#blank-cover,
    [qApp_deviceIsStorm=true] QLabel#cover {
      max-width: 126px;
      min-width: 126px;
      max-height: 196px;
      min-height: 196px;
    }
    [qApp_deviceIsDaylight=true] QLabel#blank-cover,
    [qApp_deviceIsDaylight=true] QLabel#cover {
      max-width: 140px;
      min-width: 140px;
      max-height: 218px;
      min-height: 218px;
    }

    #blank-cover {
      background-color: #d9d9d9;
    }

    SettingContainer {
      border-top: 1px solid #666666;
    }

    #first SettingContainer {
      border-top-width: 0;
    }
  )");

  QVBoxLayout *layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(0);

  SettingContainer *row = construct_SettingContainer(nullptr);
  layout->addWidget(row);
  QObject::connect(row, SIGNAL(tapped()), this, SLOT(rowTapped()));

  QHBoxLayout *rowLayout = new QHBoxLayout(row);
  rowLayout->setContentsMargins(0, 0, 0, 0);

  cover = buildCover(json);
  rowLayout->addWidget(cover);

  QVBoxLayout *textLayout = new QVBoxLayout();
  textLayout->setContentsMargins(0, 0, 0, 0);
  textLayout->setSpacing(1);
  rowLayout->addLayout(textLayout, 1);
  textLayout->addStretch(1);

  if (QWidget *widget = buildTitle(json)) {
    textLayout->addWidget(widget);
  }

  if (QWidget *widget = buildAuthor(json)) {
    textLayout->addWidget(widget);
  }

  if (QWidget *widget = buildMeta(json)) {
    textLayout->addWidget(widget);
  }

  textLayout->addStretch(1);
}

QLabel *BookRow::buildCover(QJsonObject json) {
  QLabel *label = new QLabel();
  label->setObjectName("cover");
  label->setScaledContents(true);

  QJsonValue coverUrl = json.value("coverUrl");
  if (coverUrl.isString() && !coverUrl.toString().isEmpty()) {
    label->setPixmap(QPixmap(Files::loading));

    QNetworkReply *reply = SyncController::getInstance()->network->get(QNetworkRequest(QUrl(coverUrl.toString())));
    QObject::connect(reply, &QNetworkReply::finished, this, &BookRow::loadCover);
  } else {
    label->setObjectName("blank-cover");
  }

  return label;
}

QWidget *BookRow::buildTitle(QJsonObject json) {
  QJsonValue title = json.value("title");
  if (!title.isString())
    return nullptr;

  ElidedLabel *label = new ElidedLabel(title.toString());
  label->setObjectName("large");
  return label;
}

QWidget *BookRow::buildAuthor(QJsonObject json) {
  // Bookshelf's response uses `author` (string), not Hardcover's
  // `authors` (array). isbndb rows can be a comma-joined string.
  QJsonValue author = json.value("author");
  if (!author.isString() || author.toString().isEmpty())
    return nullptr;

  ElidedLabel *label = new ElidedLabel(author.toString());
  label->setObjectName("small");
  return label;
}

QWidget *BookRow::buildMeta(QJsonObject json) {
  QStringList meta;

  QString source = json.value("source").toString();
  if (source == "isbndb") {
    // Mark isbndb hits so the user knows they're not in their library
    // yet. (v1 of the dialog doesn't create-then-link from this row -
    // user adds the book in the app first, then links.)
    meta.append("Not in your library");
  }

  QString isbn = json.value("isbn").toString();
  if (!isbn.isEmpty()) {
    meta.append("ISBN " + isbn);
  }

  double totalPages = json.value("totalPages").toDouble(0);
  if (totalPages > 0) {
    meta.append(QString::number(totalPages, 'f', 0) + " pages");
  }

  if (meta.isEmpty())
    return nullptr;

  ElidedLabel *label = new ElidedLabel(meta.join(" • "));
  label->setObjectName("small");
  return label;
}

void BookRow::loadCover() {
  QNetworkReply *reply = qobject_cast<QNetworkReply *>(sender());

  if (reply->error() == QNetworkReply::NoError) {
    QPixmap pixmap;
    pixmap.loadFromData(reply->readAll());
    cover->setPixmap(pixmap);
  } else {
    nh_log("Error loading image %s", qPrintable(reply->errorString()));
  }

  reply->deleteLater();
}

void BookRow::rowTapped() {
  // Empty bookshelfBookId = isbndb result with no library equivalent yet.
  // Surface the silent no-op so users aren't confused that the tap "did
  // nothing".
  if (bookshelfBookId.isEmpty()) {
    ConfirmationDialogFactory__showErrorDialog(
      "Bookshelf",
      "This book isn't in your library yet. Add it in the Bookshelf app first, then come back and link.");
    return;
  }
  tapped(bookshelfBookId);
}
