FROM rust:1.100.0@sha256:223cc32ddca8f3ee90c436dbc94b6b5f052ea7f9653c5201e9612f08ad7d21b8
WORKDIR /tmp

# wasm-bindgen-cli must have the same version as wasm-bindgen in Cargo.lock.
RUN cargo install --locked wasm-pack@0.15.1 && \
    cargo install --locked wasm-bindgen-cli@0.2.130 && \
    rustup target add wasm32-unknown-unknown

ADD Cargo.toml Cargo.lock ./
ADD src ./src
ADD js ./js

RUN cargo fetch --locked

ENV OUTPUT=/js
ENV INPUT=/code

CMD ["/bin/sh", "-c", "/code/compile.sh"]
