FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked -p oas-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 oas && mkdir /data && chown oas /data
COPY --from=build /src/target/release/oas-server /usr/local/bin/oas-server
USER oas
ENV OAS_DB=/data/oas.db
VOLUME /data
EXPOSE 8080
CMD ["oas-server"]
