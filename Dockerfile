FROM --platform=$BUILDPLATFORM node:24-bookworm-slim AS web
WORKDIR /build/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.98.1-slim-bookworm AS backend
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY skills/ ./skills/
COPY --from=web /build/web/dist ./web/dist/
RUN cargo build --locked --release --bins

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /build/target/release/company-server /usr/local/bin/company-server
COPY --from=backend /build/target/release/company /usr/local/bin/company
COPY --from=backend /build/target/release/company-launch /usr/local/bin/company-launch
COPY --from=web /build/web/dist /app/web
COPY THIRD_PARTY_NOTICES.md /app/THIRD_PARTY_NOTICES.md
COPY docs/licenses/ /app/docs/licenses/
ENV COMPANY_DATA_DIR=/data
EXPOSE 4177
VOLUME ["/data"]
ENTRYPOINT ["company-server"]
CMD ["--bind", "0.0.0.0:4177", "--web-dir", "/app/web"]
