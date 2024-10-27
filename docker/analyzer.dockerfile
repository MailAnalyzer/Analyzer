FROM rust:latest AS builder

WORKDIR /root
COPY src src
COPY Cargo.lock Cargo.toml ./

RUN cargo build

FROM alpine:latest AS runner

COPY --from=builder target/debug/Backend .

ENTRYPOINT Backend