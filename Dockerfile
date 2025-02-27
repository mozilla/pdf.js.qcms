FROM rust:latest
WORKDIR /tmp

RUN cargo install wasm-pack && \
    cargo install wasm-bindgen-cli && \
    rustup target add wasm32-unknown-unknown

ADD Cargo.toml .
ADD src ./src

RUN cargo update

ENV OUTPUT /js
ENV INPUT /code

CMD /code/compile.sh
