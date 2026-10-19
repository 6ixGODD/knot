ARG BASE_IMAGE=ubuntu:24.04@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517
FROM ${BASE_IMAGE} AS native
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates clang cmake curl git libssl-dev llvm \
    ninja-build pkg-config python3 python-is-python3 time unzip \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:/usr/local/go/bin:${PATH}
RUN if ! command -v rustup; then \
      curl --fail --location https://sh.rustup.rs -o /tmp/rustup.sh && \
      sh /tmp/rustup.sh -y --profile minimal --default-toolchain none; \
    fi && rustup toolchain install 1.97.1 --profile minimal --component rustfmt --component clippy \
    && rustup default 1.97.1
RUN curl --fail --location https://go.dev/dl/go1.26.4.linux-amd64.tar.gz -o /tmp/go.tar.gz \
    && echo '1153d3d50e0ac764b447adfe05c2bcf08e889d42a02e0fe0259bd47f6733ad7f  /tmp/go.tar.gz' | sha256sum -c - \
    && tar -C /usr/local -xzf /tmp/go.tar.gz && rm /tmp/go.tar.gz
ENV CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
ENV PYTHONDONTWRITEBYTECODE=1 GOTOOLCHAIN=local
WORKDIR /workspace

FROM native AS security
RUN rustup toolchain install nightly-2026-08-29 --profile minimal --component miri --component rust-src \
    && CARGO_TARGET_DIR=/tmp/fuzz-install cargo +nightly-2026-08-29 install cargo-fuzz --version 0.13.2 --locked \
    && rm -rf /tmp/fuzz-install
ENV KNOT_NIGHTLY=nightly-2026-08-29 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only

FROM ubuntu:24.04@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517 AS consumer
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake python3 python-is-python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
ENV PYTHONDONTWRITEBYTECODE=1
WORKDIR /evidence
