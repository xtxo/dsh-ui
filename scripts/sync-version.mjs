import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8');
const writeIfChanged = (rel, next) => {
  const full = path.join(root, rel);
  const prev = fs.readFileSync(full, 'utf8');
  if (prev !== next) {
    fs.writeFileSync(full, next);
    console.log(`synced ${rel}`);
  }
};

const pkg = JSON.parse(read('package.json'));
const version = pkg.version;
const tag = `v${version}`;

// Keep every public release reference derived from package.json.
// This covers README download tables and both GitHub Pages entry points.
const syncReleaseReferences = (text) => text
  .replace(
    /https:\/\/github\.com\/xtxo\/dsh-ui\/releases\/tag\/v\d+\.\d+\.\d+/g,
    `https://github.com/xtxo/dsh-ui/releases/tag/${tag}`,
  )
  .replace(
    /https:\/\/github\.com\/xtxo\/dsh-ui\/releases\/download\/v\d+\.\d+\.\d+\//g,
    `https://github.com/xtxo/dsh-ui/releases/download/${tag}/`,
  )
  .replace(/DeepSeek\.Harness_\d+\.\d+\.\d+/g, `DeepSeek.Harness_${version}`)
  .replace(/\(v\d+\.\d+\.\d+\)/g, `(${tag})`)
  .replace(/(latest-ver-badge">)v\d+\.\d+\.\d+(<)/g, `$1${tag}$2`);

for (const rel of ['README.md', 'README_EN.md', 'index.html', 'website/index.html']) {
  writeIfChanged(rel, syncReleaseReferences(read(rel)));
}

// Tauri app version.
const tauri = JSON.parse(read('src-tauri/tauri.conf.json'));
tauri.version = version;
writeIfChanged('src-tauri/tauri.conf.json', `${JSON.stringify(tauri, null, 2)}\n`);

// Rust package version.
const cargoToml = read('src-tauri/Cargo.toml').replace(
  /(\[package\][\s\S]*?\nversion = ")\d+\.\d+\.\d+("\n)/,
  `$1${version}$2`,
);
writeIfChanged('src-tauri/Cargo.toml', cargoToml);

// Cargo lockfile root package version.
const cargoLock = read('src-tauri/Cargo.lock').replace(
  /(\[\[package\]\]\nname = "deepseek-harness"\nversion = ")\d+\.\d+\.\d+("\n)/,
  `$1${version}$2`,
);
writeIfChanged('src-tauri/Cargo.lock', cargoLock);

// Runtime-facing version labels and updater comparison tag.
for (const rel of ['src-tauri/src/app/setup.rs', 'src-tauri/src/app/backend.rs']) {
  const next = read(rel).replace(/v\d+\.\d+\.\d+/g, tag);
  writeIfChanged(rel, next);
}

// Keep npm lock metadata aligned with package.json.
const lock = JSON.parse(read('package-lock.json'));
lock.version = version;
if (lock.packages?.['']) lock.packages[''].version = version;
writeIfChanged('package-lock.json', `${JSON.stringify(lock, null, 2)}\n`);

console.log(`release metadata synchronized to ${tag}`);
