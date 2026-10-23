# Web app: built only when a GitHub token with read:packages is passed as a build argument,
# because @spacecorps/components is published to the private GitHub Packages registry.
# Without the token the server still builds and serves its small fallback page.
FROM node:24-slim AS web
ARG GITHUB_TOKEN=""
WORKDIR /src/web
COPY web/package.json web/pnpm-workspace.yaml web/.npmrc web/.pnpmfile.cjs ./
COPY web/index.html web/tsconfig.json web/vite.config.ts ./
COPY web/src ./src
RUN mkdir -p /out \
 && if [ -n "$GITHUB_TOKEN" ]; then \
      corepack enable && corepack prepare pnpm@12.6.0 --activate \
      && GITHUB_TOKEN="$GITHUB_TOKEN" pnpm install --no-frozen-lockfile \
      && pnpm exec vp build && cp -R dist/. /out/; \
    else \
      echo "GITHUB_TOKEN not set: building without the web app"; \
    fi

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked -p oas-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 oas && mkdir /data && chown oas /data
COPY --from=build /src/target/release/oas-server /usr/local/bin/oas-server
COPY --from=web /out /srv/web
USER oas
ENV OAS_DB=/data/oas.db
ENV OAS_WEB_DIR=/srv/web
VOLUME /data
EXPOSE 8080
CMD ["oas-server"]
