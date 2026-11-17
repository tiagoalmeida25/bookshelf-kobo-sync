set dotenv-load

[group('helpers')]
default:
  @{{ just_executable() }} --list --justfile {{ justfile() }}

# Render modified svgs to png
[group('build')]
[working-directory('hook/res')]
build-res:
  #!/usr/bin/env sh
  for file in *.svg; do
    output="${file%.svg}.png"
    if [ ! -e "$output" -o "$file" -nt "$output" ]; then
      echo "$output"
      rsvg-convert "$file" --output "$output"
    fi
  done

# Prepare the toolchain docker image. NickelTC ships the cross-compiler and
# Qt5 headers Kobo's libnickel is built against.
[group('build')]
build-tc:
  docker build .forgejo/nickeltc --build-arg UID=$(id --user) --build-arg GID=$(id --group) --tag strayrose/nickeltc

# Cross compile both halves of the mod: the C++/Qt hook (libbookshelfsync.so)
# and the Rust sync CLI (bookshelf-kobo) for arm-unknown-linux-gnueabihf.
[group('build')]
build: build-res
  docker run --volume="$PWD:$PWD" --workdir="$PWD" --rm strayrose/nickeltc sh -c \
    "export VERSION=$(git describe --tags --long --dirty 2>/dev/null) && \
    make --directory=hook && cd cli && \
    cargo build --release --target=arm-unknown-linux-gnueabihf"

# Package the cross-compiled binaries into the installable KoboRoot.tgz the
# Kobo unpacks on boot. Layout matches what hook/src/files.h expects:
#   /usr/local/Kobo/imageformats/libbookshelfsync.so
#   /mnt/onboard/.adds/BookshelfSync/bookshelf-kobo
#   /mnt/onboard/.adds/BookshelfSync/config.ini.example
[group('package')]
package: build
  #!/usr/bin/env sh
  set -e
  rm -rf KoboRoot dist
  mkdir -p KoboRoot/usr/local/Kobo/imageformats/ KoboRoot/mnt/onboard/.adds/BookshelfSync dist
  cp hook/libbookshelfsync.so                                       KoboRoot/usr/local/Kobo/imageformats/
  cp cli/target/arm-unknown-linux-gnueabihf/release/bookshelf-kobo  KoboRoot/mnt/onboard/.adds/BookshelfSync/
  cp hook/res/config_example.ini                                    KoboRoot/mnt/onboard/.adds/BookshelfSync/config.ini.example
  tar -C KoboRoot -czf dist/KoboRoot.tgz .
  # Ship a single zip the web setup guide can link to.
  VERSION=$(git describe --tags --long --dirty 2>/dev/null || echo "dev")
  ( cd dist && \
    cp ../hook/res/config_example.ini config.ini.example && \
    zip -q "bookshelf-kobo-sync-${VERSION}.zip" KoboRoot.tgz config.ini.example && \
    cp "bookshelf-kobo-sync-${VERSION}.zip" bookshelf-kobo-sync-latest.zip )
  rm -r KoboRoot
  echo "wrote dist/bookshelf-kobo-sync-latest.zip and dist/bookshelf-kobo-sync-${VERSION}.zip"

# Copy KoboRoot.tgz onto an attached Kobo device.
[group('package')]
copy-package: package
  cp dist/KoboRoot.tgz /media/$(whoami)/KOBOeReader/.kobo/
  sudo eject /media/$(whoami)/KOBOeReader/

# Format all source files
[group('helpers')]
format:
  cd cli && cargo fmt
  clang-format -i hook/src/**/*.cc hook/src/*.cc
  clang-format -i hook/src/**/*.h hook/src/*.h

clean:
  cd hook && make clean
  cd cli && cargo clean
  rm -rf dist KoboRoot
  for f in hook/res/*.png; do git ls-files --error-unmatch $f > /dev/null 2>&1 || rm -v $f; done

# Run `logread` over ssh against the Kobo (set KOBO_SERVER + KOBO_PASSWORD
# in .env).
logs:
  #!/usr/bin/env expect
  spawn ssh $::env(KOBO_SERVER)
  expect "password: "
  send "$::env(KOBO_PASSWORD)\r"
  expect -ex {[root@kobo ~]#}
  send "logread -f\r"
  interact
