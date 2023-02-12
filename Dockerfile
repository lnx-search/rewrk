FROM rust:slim-buster as build

WORKDIR /code

COPY . /code

RUN cargo build --release

# Copy the binary into a new container for a smaller docker image
FROM debian:buster-slim

WORKDIR /etc/rewrk

COPY --from=build /code/target/release/rewrk /
USER root

ENTRYPOINT ["./rewrk"]