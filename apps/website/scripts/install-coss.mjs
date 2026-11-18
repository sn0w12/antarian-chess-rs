import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const registry = "https://coss.com/ui/r";
const here = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(here, "..");

const seen = new Set();
const dependencies = new Set();
const devDependencies = new Set();

function rewrite(content) {
  return content
    .replaceAll("@/registry/default/lib/", "@/lib/")
    .replaceAll("@/registry/default/ui/", "@/components/ui/")
    .replaceAll("@/registry/default/hooks/", "@/hooks/")
    .replaceAll("@/registry/default/", "@/");
}

function targetPath(filePath) {
  const relative = filePath
    .replace(/^registry\/default\/ui\//, "src/components/ui/")
    .replace(/^registry\/default\/lib\//, "src/lib/")
    .replace(/^registry\/default\/hooks\//, "src/hooks/")
    .replace(/^registry\/default\//, "src/");
  return path.join(appRoot, relative);
}

async function install(name) {
  if (seen.has(name)) return;
  seen.add(name);

  const response = await fetch(`${registry}/${name}.json`);
  if (!response.ok) {
    throw new Error(`${name}: ${response.status} ${response.statusText}`);
  }
  const item = await response.json();

  for (const dependency of item.dependencies ?? []) dependencies.add(dependency);
  for (const dependency of item.devDependencies ?? []) {
    devDependencies.add(dependency);
  }
  for (const dependency of item.registryDependencies ?? []) {
    await install(dependency.replace(/^@coss\//, ""));
  }

  for (const file of item.files ?? []) {
    const target = targetPath(file.path);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, rewrite(file.content));
    console.log(`wrote ${path.relative(appRoot, target)}`);
  }
}

const requested = process.argv.slice(2);
if (requested.length === 0) {
  throw new Error("pass component names, e.g. button card dialog");
}

for (const name of requested) {
  await install(name);
}

if (dependencies.size > 0) {
  console.log(`\ndependencies: ${[...dependencies].sort().join(" ")}`);
}
if (devDependencies.size > 0) {
  console.log(`devDependencies: ${[...devDependencies].sort().join(" ")}`);
}
