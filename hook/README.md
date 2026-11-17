# BookshelfSync (Kobo on-device UI mod)

This is the C++/Qt mod that runs **inside Nickel** (the Kobo native UI) and
exposes a "Bookshelf" menu, settings dialog, manual-link dialog and sync
indicator while you're reading a book. It is a fork of the
[NickelHardcover](https://codeberg.org/StrayRose/NickelHardcover) hook
(MIT licensed) with all branding swapped for Bookshelf.

The hook is intentionally a **thin Qt shell** - it does no HTTP itself.
Every interaction with the Bookshelf backend is delegated to the Rust CLI
in `../cli/`, which the hook spawns as a child process (`Files::cli =
/mnt/onboard/.adds/BookshelfSync/bookshelf-kobo`).

## Status

| Subsystem | State |
|---|---|
| Hook entry point (`bookshelfsync.cc`) | rebranded, builds against NickelHook |
| Resources (`res/*.svg`, qrc, config_example.ini) | rebranded; a handful of unused icons (left/right_star, journal status glyphs) still ship in the qrc - harmless until cleanup |
| Settings dialog (`settings/`) | rebranded; "Authorized user" row removed (Bookshelf tokens are opaque, no `get-user`) |
| Search dialog (`search/`) | ported to `koboSearchBook` response shape (no pagination, library + ISBNdb in one page); ISBNdb hits without a `bookshelfBookId` no-op with a "not in your library" hint |
| Sync controller (`synccontroller.cc`, `syncqueue.cc`) | ported - taps "Sync now" spawns `bookshelf-kobo sync`, which reads sqlite and POSTs the whole batch to `koboSyncBatch` |
| Reading status / review / rating widgets | **removed**. Kobo's native UI already writes rating + review into `KoboReader.sqlite`; our batch sync picks them up. Reading status mapping is a Phase 2.1 item if we add it back. |
| Journal dialogs | **removed** - Bookshelf has Quotes/Notes instead. |

## CLI argv shape the hook spawns

`hook/src/cli.cc` builds these argv vectors and execs `Files::cli`
(`/mnt/onboard/.adds/BookshelfSync/bookshelf-kobo`):

```
bookshelf-kobo sync                                            # SyncQueue::run
bookshelf-kobo search --include-external "<query>"             # SearchDialog
bookshelf-kobo link --kobo-content-id ID --bookshelf-book-id ID  # SearchDialog::tapped
```

The Rust CLI also supports `create-book` and `ping`, but the hook
doesn't invoke them yet (Phase 2.1 - handles ISBNdb results from the
search dialog by creating a book on the fly).

## Build

Requires [NickelTC](https://github.com/pgaskin/NickelTC) (Docker toolchain).
[NickelHook](https://github.com/pgaskin/NickelHook) is included in
`NickelHook/`. From the repository root, `just build` runs the whole build;
to build only the hook, inside the NickelTC container:

```sh
cd hook && make
```

The output is `libbookshelfsync.so` plus a `KoboRoot.tgz`.

## Install layout on the Kobo

```
/mnt/onboard/.adds/BookshelfSync/
    bookshelf-kobo          # the Rust CLI (arm-linux-gnueabihf binary)
    config.ini              # user-edited; pasted token + server url
    settings.ini            # written by the in-UI settings dialog
/usr/local/Kobo/imageformats/
    libbookshelfsync.so     # the Qt hook itself
```

## Reference

[NickelHardcover](https://codeberg.org/StrayRose/NickelHardcover) is the
upstream this was forked from. Diff against it when porting controller logic.
