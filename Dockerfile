FROM rust:1.40 as builder

WORKDIR /src/
COPY . .
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --features=container --target=x86_64-unknown-linux-musl


FROM scratch

ENTRYPOINT [ "/usr/local/bin/doxie-upload" ]
CMD [ "--root=/uploads", "--port=8080", "--address=0.0.0.0", "-v" ]
EXPOSE 8080/tcp
VOLUME /uploads
COPY --from=builder /src/target/x86_64-unknown-linux-musl/release/doxie-upload /usr/local/bin/doxie-upload
