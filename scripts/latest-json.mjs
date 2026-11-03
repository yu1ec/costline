#!/usr/bin/env node
// 根据 Tauri 产出的 .app.tar.gz 与 .sig 生成 updater 使用的 latest.json。
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const options = {
  bundleDir: "target/release/bundle",
  version: "",
  repo: "yu1ec/costline",
  tag: "",
  notes: process.env.RELEASE_NOTES ?? "",
  out: "",
  universal: false,
};

const argv = process.argv.slice(2);
for (let index = 0; index < argv.length; index += 1) {
  const flag = argv[index];
  const value = argv[index + 1];
  switch (flag) {
    case "--bundle-dir":
      options.bundleDir = value;
      index += 1;
      break;
    case "--version":
      options.version = value;
      index += 1;
      break;
    case "--repo":
      options.repo = value;
      index += 1;
      break;
    case "--tag":
      options.tag = value;
      index += 1;
      break;
    case "--notes":
      options.notes = value;
      index += 1;
      break;
    case "--out":
      options.out = value;
      index += 1;
      break;
    case "--universal":
      options.universal = true;
      break;
    case "--help":
    case "-h":
      printHelp();
      process.exit(0);
    default:
      if (flag.startsWith("--")) {
        fail(`未知参数：${flag}`);
      }
  }
}

if (!options.version) {
  const packageJson = JSON.parse(await readFile("package.json", "utf8"));
  options.version = packageJson.version;
}
options.tag ||= `v${options.version}`;
options.out ||= path.join(options.bundleDir, "latest.json");

const artifacts = (await walk(options.bundleDir)).filter((file) =>
  file.endsWith(".app.tar.gz"),
);

if (artifacts.length === 0) {
  fail(`没有在 ${options.bundleDir} 下找到 .app.tar.gz，请先运行 make package`);
}

const platforms = {};
for (const artifact of artifacts) {
  const signaturePath = `${artifact}.sig`;
  let signature;
  try {
    signature = (await readFile(signaturePath, "utf8")).trim();
  } catch {
    console.warn(`跳过没有签名的产物：${artifact}`);
    continue;
  }
  if (!signature) {
    console.warn(`跳过签名文件为空的产物：${artifact}`);
    continue;
  }

  const name = path.basename(artifact);
  const keys = platformKeys(name, options.universal);
  if (keys.length === 0) {
    console.warn(`无法从文件名判断架构，跳过：${name}`);
    continue;
  }

  const url = `https://github.com/${options.repo}/releases/download/${encodeURIComponent(
    options.tag,
  )}/${encodeURIComponent(name)}`;
  for (const key of keys) {
    platforms[key] = { signature, url };
  }
}

if (Object.keys(platforms).length === 0) {
  fail("没有生成任何平台条目，检查产物文件名与 .sig 文件");
}

const manifest = {
  version: options.version,
  notes: options.notes,
  pub_date: new Date().toISOString(),
  platforms,
};

await writeFile(options.out, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`==> 已生成 ${options.out}`);
console.log(
  `    platforms: ${Object.keys(platforms)
    .map((key) => `${key} -> ${path.basename(new URL(platforms[key].url).pathname)}`)
    .join(", ")}`,
);

function platformKeys(name, universal) {
  if (universal || /universal/i.test(name)) {
    return ["darwin-aarch64", "darwin-x86_64"];
  }
  if (/aarch64|arm64/i.test(name)) return ["darwin-aarch64"];
  if (/x86_64|x64/i.test(name)) return ["darwin-x86_64"];
  // 单架构本地打包时 Tauri 不会在文件名里带架构，按当前机器判断。
  if (process.arch === "arm64") return ["darwin-aarch64"];
  if (process.arch === "x64") return ["darwin-x86_64"];
  return [];
}

async function walk(directory) {
  const files = [];
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    return files;
  }
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await walk(fullPath)));
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }
  return files;
}

function fail(message) {
  console.error(`latest-json: ${message}`);
  process.exit(1);
}

function printHelp() {
  console.log(`用法：node scripts/latest-json.mjs [选项]

  --bundle-dir <dir>   Tauri bundle 目录（默认 target/release/bundle）
  --version <x.y.z>    更新版本号（默认读取 package.json）
  --repo <owner/repo>  GitHub 仓库（默认 yu1ec/costline）
  --tag <tag>          Release tag（默认 v<version>）
  --notes <text>       更新说明
  --universal          通用二进制，同时写入 aarch64 / x86_64 两个平台
  --out <file>         输出路径（默认 <bundle-dir>/latest.json）`);
}
