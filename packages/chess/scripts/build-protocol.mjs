import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(here, "..");
const repoRoot = path.resolve(packageRoot, "../..");

const result = spawnSync(
    "cargo",
    [
        "run",
        "-q",
        "-p",
        "chess_server",
        "--features",
        "export-ts",
        "--bin",
        "export-ts",
        "--",
        path.join(packageRoot, "src"),
    ],
    { cwd: repoRoot, stdio: "inherit" }
);

if (result.error) {
    console.error(result.error.message);
    process.exit(1);
}
process.exit(result.status ?? 1);
