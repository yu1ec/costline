#!/usr/bin/env node
// 把同一份版本号写入 package.json、Cargo.toml、Cargo.lock 与 tauri.conf.json。
import { readFile, writeFile } from "node:fs/promises";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error("用法：node scripts/set-version.mjs x.y.z");
  process.exit(1);
}

const changes = [];

await updateJson("package.json", (data) => {
  data.version = version;
});
await updateJson("package-lock.json", (data) => {
  data.version = version;
  if (data.packages?.[""]) data.packages[""].version = version;
});
await updateJson("src-tauri/tauri.conf.json", (data) => {
  data.version = version;
});
await updateText("Cargo.toml", (text) =>
  replaceOnce(
    text,
    /(\[workspace\.package\][\s\S]*?\nversion = ")[^"]+(")/,
    (_, prefix, suffix) => `${prefix}${version}${suffix}`,
    "Cargo.toml workspace.package.version",
  ),
);
await updateText("Cargo.lock", (text) =>
  text.replace(
    /(\[\[package\]\]\nname = "(?:costline-core|costline-cli|costline-desktop)"\nversion = ")[^"]+(")/g,
    `$1${version}$2`,
  ),
);

const packageJson = JSON.parse(await readFile("package.json", "utf8"));
const tauriConfig = JSON.parse(
  await readFile("src-tauri/tauri.conf.json", "utf8"),
);
console.log(`==> 版本号已更新为 ${packageJson.version}（tauri.conf.json: ${tauriConfig.version}）`);
for (const change of changes) {
  console.log(`    ${change}`);
}

async function updateJson(file, mutate) {
  const data = JSON.parse(await readFile(file, "utf8"));
  mutate(data);
  await writeFile(file, `${JSON.stringify(data, null, 2)}\n`, "utf8");
  changes.push(file);
}

async function updateText(file, mutate) {
  const before = await readFile(file, "utf8");
  const after = mutate(before);
  if (after !== before) {
    await writeFile(file, after, "utf8");
    changes.push(file);
  }
}

function replaceOnce(text, pattern, replacement, label) {
  if (!pattern.test(text)) {
    throw new Error(`没有找到 ${label}，版本号未写入`);
  }
  return text.replace(pattern, replacement);
}
