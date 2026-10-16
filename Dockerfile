FROM --platform=linux/amd64 rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build
WORKDIR /src
ADD --checksum=sha256:121f6c7afe1d4d0e3ea6aab9432038599250134cbf4474cb1167d2c7decd4278 https://github.com/protocolbuffers/protobuf/releases/download/v36.2/protoc-36.2-linux-x86_64.zip /tmp/protoc.zip
RUN unzip /tmp/protoc.zip -d /opt/protoc && /opt/protoc/bin/protoc --version
ENV PROTOC=/opt/protoc/bin/protoc CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0
COPY . .
RUN --mount=type=cache,id=knot-server-registry,sharing=locked,target=/usr/local/cargo/registry \
    --mount=type=cache,id=knot-server-build,sharing=locked,target=/src/target \
    cargo build -p knot-server --release --locked && cp target/release/knot-server /knot-server && python3 tools/server/notices.py --out /third-party

FROM --platform=linux/amd64 debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG SOURCE_COMMIT
ARG WORKSPACE_VERSION
LABEL org.opencontainers.image.revision=$SOURCE_COMMIT org.opencontainers.image.version=$WORKSPACE_VERSION
COPY --from=build /knot-server /usr/local/bin/knot-server
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY contracts/grpc/ /usr/share/knot/protocol/
COPY --from=build /third-party/ /usr/share/licenses/knot/third-party/
COPY tools/server/license-status.json /usr/share/licenses/knot/license-status.json
WORKDIR /app
USER 10001:10001
ENV KNOT_LISTEN=0.0.0.0:50051 KNOT_MODEL_DIR=/models
EXPOSE 50051
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD ["/usr/local/bin/knot-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/knot-server"]
CMD ["serve"]
