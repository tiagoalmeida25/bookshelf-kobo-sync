#include <QJsonDocument>
#include <QProcess>
#include <QTimer>

#include <NickelHook.h>

#include "cli.h"
#include "files.h"
#include "settings.h"
#include "synccontroller.h"

CLI *CLI::sync(bool silent) {
  QStringList arguments = {"sync"};
  return new CLI(arguments, silent);
}

CLI *CLI::search(QString query) {
  // `--include-external` makes the server also query ISBNdb when nothing
  // matches in the library - important because users will typically reach
  // for the manual-link dialog on books that aren't in their library yet.
  return new CLI({"search", "--include-external", query});
}

CLI *CLI::link(QString koboContentId, QString bookshelfBookId) {
  return new CLI({
    "link",
    "--kobo-content-id",
    koboContentId,
    "--bookshelf-book-id",
    bookshelfBookId,
  });
}

CLI::CLI(QStringList arguments, bool silent, QObject *parent) : QObject(parent), arguments(arguments), silent(silent) {
  WirelessWorkflowManager *wfm = WirelessWorkflowManager__sharedInstance();

  if (WirelessWorkflowManager__isInternetAccessible(wfm)) {
    networkConnected();
  } else {
    MainWindowController *mwc = MainWindowController__sharedInstance();
    QObject::connect(wfm, SIGNAL(connectingFailed()), this, SLOT(connectingFailed()));

    QWidget *window = MainWindowController__currentView(mwc)->window();
    wifiIcon = new QLabel(window);
    wifiIcon->setPixmap(QPixmap(Files::wifi));
    wifiIcon->resize(90, 90);
    wifiIcon->move(window->width() - 144, window->height() - 144);
    wifiIcon->show();

    timer = new QTimer(this);
    connect(timer, &QTimer::timeout, this, &CLI::connectingFailed);
    timer->setSingleShot(true);
    timer->start(30000);

    WirelessManager *wm = WirelessManager__sharedInstance();
    QObject::connect(wm, SIGNAL(networkConnected()), this, SLOT(networkConnected()));

    if (silent) {
      WirelessWorkflowManager__connectWirelessSilently(wfm);
    } else {
      WirelessWorkflowManager__connectWireless(wfm, false, false);
    }
  }
}

void CLI::connectingFailed() {
  nh_log("CLI::connectingFailed()");

  if (!silent) {
    ConfirmationDialogFactory__showErrorDialog("Bookshelf", "Failed to connect to WIFI.");
  }

  if (timer != nullptr) {
    timer->stop();
    timer->deleteLater();
    timer = nullptr;
  }

  if (wifiIcon != nullptr) {
    wifiIcon->deleteLater();
    wifiIcon = nullptr;
  }
  deleteLater();
  failure();
}

void CLI::networkConnected() {
  nh_log("CLI::networkConnected()");

  if (timer != nullptr) {
    timer->stop();
    timer->deleteLater();
    timer = nullptr;
  }

  if (wifiIcon != nullptr) {
    wifiIcon->deleteLater();
    wifiIcon = nullptr;
  }

  QProcess *process = new QProcess(this);
  process->start(Files::cli, arguments);
  QObject::connect(process, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), this, &CLI::processFinished);
}

void CLI::processFinished(int exitCode) {
  QProcess *process = qobject_cast<QProcess *>(sender());
  deleteLater();

  QByteArray stdout = process->readAllStandardOutput();

  if (exitCode > 0) {
    QByteArray stderr = process->readAllStandardError();
    nh_log("Error from command line \"%s\"", qPrintable(stderr));
    if (!silent) {
      ConfirmationDialogFactory__showErrorDialog("Bookshelf", QString(stderr));
    }
    failure();
    return;
  }

  // The Bookshelf CLI prints a single JSON document to stdout (no
  // BEGIN_JSON sentinel - the Hardcover CLI mixed log lines and JSON in
  // the same stream; ours doesn't).
  QJsonParseError parseError;
  QJsonDocument doc = QJsonDocument::fromJson(stdout, &parseError);
  if (parseError.error == QJsonParseError::NoError && doc.isObject()) {
    response(doc.object());
  } else if (!stdout.isEmpty()) {
    // Log non-JSON stdout for diagnostics but don't fail - `sync` prints
    // pretty-printed JSON which is still parseable; if it isn't, the user
    // sees the raw response in the log.
    nh_log("CLI stdout (non-JSON or empty object): %s", stdout.constData());
  }

  success();
}
