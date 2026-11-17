#include <QLabel>
#include <QPixmap>
#include <QWidgetAction>

#include <NickelHook.h>

#include "cli.h"
#include "files.h"
#include "menucontroller.h"
#include "search/searchdialog.h"
#include "settings.h"
#include "settings/settingsdialog.h"
#include "synccontroller.h"

MenuController::MenuController(QWidget *parent) : QWidget(parent) {
  icon = construct_TouchLabel(parent);
  TouchLabel__setHitStateEnabled(icon, false);
  icon->setPixmap(QPixmap(Files::icon));

  QWidget::connect(icon, SIGNAL(tapped(bool)), this, SLOT(showMainMenu(bool)));
};

void MenuController::showMainMenu(bool checked) {
  nh_log("MenuController::showMainMenu(%s)", checked ? "true" : "false");

  icon->setPixmap(QPixmap(Files::icon_hit));

  NickelTouchMenu *menu = construct_NickelTouchMenu(icon);
  NickelTouchMenu__showDecoration(menu, true);
  QWidget::connect(menu, &QMenu::aboutToHide, menu, &QWidget::deleteLater);
  QWidget::connect(menu, &QMenu::aboutToHide, icon, [this] { icon->setPixmap(QPixmap(Files::icon)); });

  // Bookshelf's sync covers progress, highlights, ratings and reviews in a
  // single batch read from KoboReader.sqlite. The reading status, rating
  // and review widgets that NickelHardcover injected here are not needed:
  // Kobo's own UI already writes those into sqlite, and our sync picks
  // them up on the next pass.
  QWidgetAction *action = addMenuItem(menu, "Sync now");
  QObject::connect(action, &QAction::triggered, this, &MenuController::syncNow);

  menu->addSeparator();

  QString contentId = SyncController::getInstance()->contentId;

  action = addMenuItem(menu, Settings::getInstance()->isEnabled(contentId) ? "Disable auto-sync" : "Enable auto-sync");
  QObject::connect(action, &QAction::triggered, this, &MenuController::toggleEnabled);

  menu->addSeparator();

  action = addMenuItem(menu, Settings::getInstance()->getLinkedBook(contentId).isEmpty() ? "Manually link book"
                                                                                         : "Unlink book");
  QObject::connect(action, &QAction::triggered, this, &MenuController::linkBook);

  menu->addSeparator();

  action = addMenuItem(menu, "Settings");
  QObject::connect(action, &QAction::triggered, this, &MenuController::openSettings);

  menu->ensurePolished();
  menu->popup(icon->mapToGlobal(icon->geometry().bottomRight()) + QPoint(0, 6));
}

QWidgetAction *MenuController::addMenuItem(NickelTouchMenu *menu, QString label, bool checkable, bool checked) {
  MenuTextItem *item = construct_MenuTextItem(menu, checkable, true);
  MenuTextItem__setText(item, label);
  MenuTextItem__registerForTapGestures(item);

  if (checked) {
    MenuTextItem__setSelected(item, true);
  }

  QWidgetAction *action = new QWidgetAction(menu);
  action->setDefaultWidget(item);
  action->setEnabled(true);
  menu->addAction(action);

  QObject::connect(action, &QAction::triggered, menu, &QMenu::hide);
  QObject::connect(item, SIGNAL(tapped(bool)), action, SIGNAL(triggered()));

  return action;
}

void MenuController::syncNow(bool checked) {
  nh_log("MenuController::syncNow(%s)", checked ? "true" : "false");

  SyncController::getInstance()->manualSync();
}

void MenuController::toggleEnabled(bool checked) {
  nh_log("MenuController::toggleEnabled(%s)", checked ? "true" : "false");

  QString contentId = SyncController::getInstance()->contentId;
  Settings::getInstance()->setEnabled(contentId, !Settings::getInstance()->isEnabled(contentId));
}

void MenuController::linkBook(bool checked) {
  nh_log("MenuController::linkBook(%s)", checked ? "true" : "false");

  SyncController *ctl = SyncController::getInstance();

  if (Settings::getInstance()->getLinkedBook(ctl->contentId).isEmpty()) {
    SearchDialog::show(ctl->title + " " + ctl->author);
  } else {
    Settings::getInstance()->setLinkedBook(ctl->contentId, QString());
  }
}

void MenuController::openSettings(bool checked) {
  nh_log("MenuController::openSettings(%s)", checked ? "true" : "false");

  SettingsDialog::show();
}
