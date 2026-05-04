FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app

# Copy only the server crate (standalone — no path dependencies)
COPY crates/server/ .
# Copy the lock file for reproducible dependency resolution
COPY Cargo.lock /app/

RUN cargo build --release --target x86_64-unknown-linux-musl
RUN strip /app/target/x86_64-unknown-linux-musl/release/chess_server

FROM scratch
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/chess_server /chess_server
ENV PORT=20682
EXPOSE 20682
CMD ["/chess_server"]
