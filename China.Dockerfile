FROM rust:latest as builder

USER root
WORKDIR /workdir
COPY ./cargo_config.toml $CARGO_HOME/config
COPY . .

RUN cargo build --release

##################################

FROM debian:stable as production

RUN apt update && apt install -y git openssl ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=0 /workdir/target/release/lernaean /usr/bin/lernaean

