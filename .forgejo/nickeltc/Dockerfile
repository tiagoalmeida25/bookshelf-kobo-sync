FROM ghcr.io/pgaskin/nickeltc:1.0-9779f94.19
ARG UID=0
ARG GID=0
# Reuse the GID if the base image already has it (macOS hosts pass
# GID=20 which clashes with debian's `dialout` group).
RUN if [ $UID -ne 0 ]; then \
      if ! getent group $GID >/dev/null; then groupadd --gid $GID user; fi && \
      adduser --uid $UID --gid $GID --disabled-password --gecos "" user && \
      mkdir /rustup /cargo && chown $UID:$GID /rustup /cargo; \
    fi
USER $UID
ENV RUSTUP_HOME=/rustup CARGO_HOME=/cargo
RUN mkdir -p /rustup /cargo && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --target arm-unknown-linux-gnueabihf
ENV PATH=$CARGO_HOME/bin:$PATH
