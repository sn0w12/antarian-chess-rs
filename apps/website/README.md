# Antarian Chess website

Vite, React, and Tailwind app with coss UI components. The board draws with
plain HTML and CSS; the engine, bot, and online protocol come from the
`antarian-chess` package.

## Development

Build the package once so the `file:../../packages/chess` dependency has its
`dist` output:

```sh
cd packages/chess
npm install
npm run build
```

Then run the site:

```sh
cd apps/website
npm install
npm run dev
```

The engine wasm is served from `public/chess_wasm.wasm`. Refresh it after a
package rebuild with `npm run sync:wasm`.

## Docker

Build from the repository root so the Rust crates and package sources are in
the build context:

```sh
docker build -f apps/website/Dockerfile -t antarian-chess-web .
docker run --rm -p 8080:80 antarian-chess-web
```

The image builds the wasm engine in a Rust stage, builds the package and site
in a Node stage, and serves the SPA from nginx with history fallback.
