FROM rust:slim-buster as build

WORKDIR /code

COPY . /code

RUN cargo build --release

# Copy the binary into a new container for a smaller docker image
FROM debian:buster-slim

WORKDIR /etc/rewrk

USER root
RUN apt-get update \
    && apt-get install -y ca-certificates libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/* \

COPY --from=build /code/target/release/rewrk /

ENTRYPOINT ["./rewrk"]