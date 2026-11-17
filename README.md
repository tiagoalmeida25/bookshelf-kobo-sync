# Bookshelf Kobo Sync

A mod for Kobo eReaders that sends your reading progress, highlights, notes,
ratings and reviews to [Bookshelf](https://bookshelf.tcreations.app), a reading
tracker for iOS and Android.

**[Setup guide and download](https://bookshelf.tcreations.app/kobo-sync)**

It is a fork of [NickelHardcover](https://codeberg.org/StrayRose/NickelHardcover)
by StrayRose, built on [NickelHook](https://github.com/pgaskin/NickelHook) by
Patrick Gaskin. Both are MIT licensed, and so is this.

This is an independent project. It is not affiliated with or endorsed by
Rakuten Kobo, and Kobo does not support it. It changes software on your device,
so you install it at your own risk, and a Kobo firmware update can stop it
working.

## What it does

After a reading session, once the Kobo is on Wi-Fi, the mod uploads:

- reading progress for each session
- highlights, which become Quotes in Bookshelf
- annotations, which become Notes
- star ratings and reviews written on the device

Sync is one-way, from the Kobo to Bookshelf. Nothing is written back to the
device, and nothing goes through Goodreads or any other service. A
"Bookshelf > Sync now" entry in the Kobo menu runs a sync by hand.

Books are matched to your library by ISBN. A book without one, such as a
sideloaded EPUB, shows up as unmatched in the app, where you link it once.

## Requirements

- A Kobo eReader running the stock Kobo software (Nickel).
- The Bookshelf app with a Premium subscription. The sync token is generated in
  the app under Settings > Customization > Kobo sync, and that needs Premium.
- A USB cable and a computer, to copy two files onto the Kobo.

## Install

The [setup guide](https://bookshelf.tcreations.app/kobo-sync) walks through it
with troubleshooting. In short:

1. Generate a sync token in the app.
2. Copy `KoboRoot.tgz` into the hidden `.kobo/` folder on the Kobo.
3. Create `.adds/BookshelfSync/config.ini` from `config.ini.example` and paste
   the token into it.
4. Eject. The Kobo installs the mod and restarts.

To uninstall, create an empty file called `bookshelfsync_uninstall` in the root
of the Kobo drive and eject. The mod removes itself on the next boot.

## How it is put together

- `hook/` is the C++/Qt part that runs inside Nickel, the Kobo's own interface.
  It adds the menu, the settings dialog and the dialog for linking a book by
  hand. It does no networking itself. See [hook/README.md](hook/README.md).
- `cli/` is a small Rust program the hook starts. It reads the Kobo's reading
  database and sends the changes to the Bookshelf backend over HTTPS.
- `dist/` holds the packaged build that the setup guide links to.
- `LICENSES.txt` holds the licence notices for the mod and the Rust libraries
  built into the sync program. It ships inside the download. Regenerate it with
  `python3 scripts/third_party_licenses.py` when `cli/Cargo.lock` changes.

## Build

Both halves cross-compile inside the [NickelTC](https://github.com/pgaskin/NickelTC)
Docker toolchain, the same one NickelHardcover uses. With
[just](https://github.com/casey/just) and Docker installed:

```sh
just build-tc   # build the toolchain image (once)
just package    # build the hook and the CLI, write dist/KoboRoot.tgz
```

To build only the Rust CLI by hand:

```sh
# inside the NickelTC Docker container, with this repository mounted:
cd cli
cargo build --release --target arm-unknown-linux-gnueabihf
```

The resulting binary is installed as `.adds/BookshelfSync/bookshelf-kobo` on the
device.

## Configuration

Users paste a sync token (generated in the Bookshelf app's Settings → Kobo
sync page) into `.adds/BookshelfSync/config.ini` on the Kobo:

```ini
; The sync token generated in the Bookshelf app (Settings → Kobo sync).
authorization = bks_<uid>_<48hex>

; Base URL of the Bookshelf Cloud Functions (region-specific). Override if
; you're running against a different Firebase project.
server_url = https://us-central1-bookshelf-a829b.cloudfunctions.net

; Conflict resolution when Bookshelf already has more progress than Kobo.
; - kobo       Push Kobo's progress anyway (you re-read on Kobo)
; - bookshelf  Skip the page entry (Bookshelf stays authoritative)
; - auto       Take the max - only push if Kobo is ahead (default)
conflict_resolution = auto

; When to sync Kobo highlights/annotations.
; - always    Sync bookmarks with every sync
; - never     Never sync bookmarks
; - finished  Only sync bookmarks when reading progress reaches 100%
sync_bookmarks = always
```

### Sharing one Kobo between two readers

A Kobo is often shared by a couple who each want their own reading history.
Add the second reader's token and the device pushes each book to whichever
account it belongs to:

```ini
authorization   = bks_<uidA>_<48hex>
authorization_2 = bks_<uidB>_<48hex>

; Who gets a book neither reader has claimed: 1 or 2.
default_account = 1
```

Each account needs its own Premium subscription, because generating a token
requires one.

Setup order matters: configure both tokens and run one sync **first**. That
sync calls `koboRegisterDevicePeers`, which is what tells each account about
the other; only then does the per-book switch appear in the app under
Settings → Kobo sync. Until a book is assigned it goes to `default_account`.

How a book is routed, given each account's choices:

| Account A says | Account B says | Goes to |
|---|---|---|
| mine | *(nothing)* | A |
| B's | *(nothing)* | B |
| mine | mine | **both** - two people really are reading the same copy |
| *(nothing)* | *(nothing)* | `default_account` |

The merge happens on the device, because it is the only party holding both
tokens. Each batch is still sent with its own account's bearer token, so the
server side is unchanged: a write can only ever land in the library the
token belongs to, and neither reader can write into the other's.

## Architecture

```
┌─────────────────────────────────┐
│ Kobo device (libnickel + hook)  │
│ ┌─────────────────────────────┐ │
│ │ Bookshelf hook (C++/Qt)     │ │     spawn
│ │ - menu injection            │─┼──────────────┐
│ │ - sync trigger              │ │              │
│ │ - settings dialog           │ │              ▼
│ └─────────────────────────────┘ │     ┌──────────────────┐
│                                 │     │ Bookshelf CLI    │
│ /mnt/onboard/.kobo/             │◀────│ (Rust binary)    │
│   KoboReader.sqlite ────────────┼─────│  reads sqlite    │
│                                 │     │  POST /koboSync* │
└─────────────────────────────────┘     └────────┬─────────┘
                                                  │ HTTPS, Bearer <token>
                                                  ▼
                                         ┌──────────────────┐
                                         │ Firebase         │
                                         │ Cloud Functions  │
                                         │  koboSyncBatch   │
                                         │  koboSearchBook  │
                                         │  koboLinkBook    │
                                         │  koboCreateBook  │
                                         └────────┬─────────┘
                                                  │
                                                  ▼
                                         ┌──────────────────┐
                                         │ Firestore        │
                                         │  user/{uid}/     │
                                         │    books/        │
                                         │    kobo_links/   │
                                         │    kobo_sync_log │
                                         └──────────────────┘
```

## Privacy

- The Kobo starts every sync and talks directly to the Bookshelf backend over
  HTTPS.
- The sync token is stored in plain text in `config.ini` on the Kobo. The
  server keeps only its SHA-256 hash.
- A token can be revoked in the app at any time and stops working immediately.

## Questions and bugs

Open an issue here, or email support@tcreations.app. Please include your Kobo
model and firmware version.

## License

MIT. See [LICENSE](LICENSE). This project keeps the copyright notice of
NickelHardcover, which it is forked from. `hook/NickelHook/` is NickelHook by
Patrick Gaskin, under its own MIT license in that folder.
