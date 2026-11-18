FROM rust:bookworm AS wasm

RUN rustup target add wasm32-unknown-unknown
WORKDIR /src

COPY crates/engine crates/engine
COPY crates/bot crates/bot
COPY crates/wasm crates/wasm
RUN printf '[workspace]\nmembers = ["crates/engine", "crates/bot", "crates/wasm"]\nresolver = "3"\n' > Cargo.toml

RUN cargo build --release -p chess_wasm --target wasm32-unknown-unknown
RUN mkdir /out && cp target/wasm32-unknown-unknown/release/chess_wasm.wasm /out/chess_wasm.wasm


FROM node:24-bookworm-slim AS build

WORKDIR /app

COPY packages/chess packages/chess
RUN cd packages/chess && npm ci && npx tsc -p tsconfig.json
COPY --from=wasm /out/chess_wasm.wasm packages/chess/dist/chess_wasm.wasm

COPY apps/website apps/website
COPY --from=wasm /out/chess_wasm.wasm apps/website/public/chess_wasm.wasm
RUN cd apps/website && npm ci && npm run build


FROM nginx:alpine AS runtime

COPY apps/website/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/apps/website/dist /usr/share/nginx/html

EXPOSE 80
