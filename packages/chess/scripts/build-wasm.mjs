import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(here, "..");
const repoRoot = path.resolve(packageRoot, "../..");

const build = spawnSync(
    "cargo",
    [
        "build",
        "-p",
        "chess_wasm",
        "--target",
        "wasm32-unknown-unknown",
        "--release",
    ],
    { cwd: repoRoot, stdio: "inherit" }
);

if (build.error) {
    console.error(build.error.message);
    process.exit(1);
}
if (build.status !== 0) {
    process.exit(build.status ?? 1);
}

const source = path.join(
    repoRoot,
    "target/wasm32-unknown-unknown/release/chess_wasm.wasm"
);
const outDir = path.join(packageRoot, "dist");
fs.mkdirSync(outDir, { recursive: true });
const destination = path.join(outDir, "chess_wasm.wasm");
fs.copyFileSync(source, destination);

console.log(`copied ${path.relative(repoRoot, destination)}`);
