import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(here, "..");
const repoRoot = path.resolve(appRoot, "../..");

const candidates = [
  path.join(repoRoot, "packages/chess/dist/chess_wasm.wasm"),
  path.join(
    repoRoot,
    "target/wasm32-unknown-unknown/release/chess_wasm.wasm",
  ),
];

const source = candidates.find((candidate) => fs.existsSync(candidate));
if (!source) {
  console.error(
    "chess_wasm.wasm not found. Build it first: cd packages/chess && npm run build:wasm",
  );
  process.exit(1);
}

const destination = path.join(appRoot, "public/chess_wasm.wasm");
fs.mkdirSync(path.dirname(destination), { recursive: true });
fs.copyFileSync(source, destination);
console.log(`copied ${path.relative(appRoot, destination)}`);
