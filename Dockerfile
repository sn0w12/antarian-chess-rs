FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app

COPY crates/server/ .
COPY Cargo.lock /app/

RUN cargo build --release --target x86_64-unknown-linux-musl
RUN strip /app/target/x86_64-unknown-linux-musl/release/chess_server

# ---- Final image ----
FROM alpine:latest
RUN apk add --no-cache nginx

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/chess_server /usr/local/bin/chess_server
COPY nginx.conf /etc/nginx/nginx.conf

COPY start.sh /start.sh
RUN chmod +x /start.sh

ENV PORT=20683
EXPOSE 20682

CMD ["/start.sh"]