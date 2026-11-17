#include <QObject>
#include <QSettings>
#include <QWidgetAction>

#include "bookshelfsync.h"

class MenuController : public QWidget {
  Q_OBJECT

public:
  MenuController(QWidget *parent = nullptr);
  TouchLabel *icon = nullptr;

public Q_SLOTS:
  void showMainMenu(bool checked);

  void syncNow(bool checked);
  void toggleEnabled(bool checked);
  void linkBook(bool checked);
  void openSettings(bool checked);

private:
  QWidgetAction *addMenuItem(NickelTouchMenu *menu, QString label, bool checkable = false, bool checked = false);
};
