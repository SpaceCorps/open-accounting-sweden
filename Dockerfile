# Web app. @spacecorps/components is built from the public SpaceCorps/storybook repository at a
# pinned commit, so no registry token is needed. Bump STORYBOOK_REF to pick up library changes.
FROM node:24-slim AS web
ARG STORYBOOK_REF=5a20a4c39c31351939ba371b0749d197c4bdc27b
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
 && rm -rf /var/lib/apt/lists/* && corepack enable
RUN git clone https://github.com/SpaceCorps/storybook.git /storybook \
 && git -C /storybook checkout --quiet "$STORYBOOK_REF"
WORKDIR /storybook
RUN pnpm install --frozen-lockfile && pnpm exec vp pack
WORKDIR /src/web
COPY web/package.json web/pnpm-workspace.yaml web/.pnpmfile.cjs ./
RUN OAS_STORYBOOK_PATH=/storybook pnpm install --no-frozen-lockfile
COPY web/index.html web/tsconfig.json web/vite.config.ts ./
COPY web/src ./src
RUN pnpm exec vp build

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked -p oas-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 oas && mkdir /data && chown oas /data
COPY --from=build /src/target/release/oas-server /usr/local/bin/oas-server
COPY --from=web /src/web/dist /srv/web
USER oas
ENV OAS_DB=/data/oas.db
ENV OAS_WEB_DIR=/srv/web
VOLUME /data
EXPOSE 8080
CMD ["oas-server"]
