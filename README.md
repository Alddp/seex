<p align="center">
  <img src="src/assets/seex.svg" alt="SeEx Logo" width="200" />
</p>

<h1 align="center">SeEx</h1>

<p align="center">
  <strong>Seek &amp; Export</strong> — Clipboard Event Tracker
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.3.3-E8656C" alt="version" />
  <img src="https://img.shields.io/badge/license-CC%20BY--NC%204.0-F4845F" alt="license" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-66BB6A" alt="platform" />
</p>

---

## What is SeEx?

SeEx monitors your clipboard in real time, extracts component IDs such as LCSC C-codes using smart pattern matching, and exports matched results through `nlbn` or `npnp`. It is built for electronics engineers working with component databases and library export workflows.

---

## How to Use

### 1. Monitor — Match component IDs from clipboard

<p align="center">
  <img src="public/imgs/Monitor.png" alt="Monitor Page" width="700" />
</p>

The **Monitor** page is where the magic happens:

- **Quick ID** — Toggle this to match when you directly copy a bare component ID like `C7470135`.
- **Full Info** — Toggle this to extract the C-code when you copy a full component info block containing `编号：C7470135`.
- **Monitoring** — Green means active. Click to pause or resume clipboard monitoring.
- Both **Quick ID** and **Full Info** can be enabled at the same time, so every copy is covered.
- The **Matched** panel shows all unique extracted IDs with timestamps. Use **Copy IDs** to copy them all, or click **Copy** on individual items.
- The **Clipboard** panel previews the latest copied content.
- **Save History** / **Export Matched** / **Clear All** buttons are pinned at the bottom.

> **Tip:** Duplicate IDs are automatically filtered — copying the same component multiple times will not create duplicates.

---

### 2. History — View all clipboard activity

<p align="center">
  <img src="public/imgs/History.png" alt="History Page" width="700" />
</p>

The **History** page shows every clipboard entry captured while monitoring is active. Each entry shows the timestamp and a preview of the copied content. You can **Copy** or **Delete** individual entries. Up to 50 entries are stored.

---

### 3. Export — Batch export via nlbn and npnp

<p align="center">
  <img src="public/imgs/Export.png" alt="Export Page" width="700" />
</p>

The **Export** page supports two export backends:

#### `nlbn` export

- Set the **export directory** by typing a path or clicking **Browse**.
- Click **Apply** to save the path.
- **Toggle Terminal** switches between opening a terminal window to view `nlbn` output or running silently in the background.
- Adjust **Parallel jobs** to control the `--parallel` argument.
- Click **Export nlbn** to batch-export all matched component IDs.

#### `npnp` export

- Set the **export directory** independently from `nlbn`.
- Choose **Full**, **SchLib**, or **PcbLib** export mode.
- Toggle batch options such as **Merge**, **Continue On Error**, and **Force**.
- Set a **Library name** when merge mode is enabled.
- Adjust **Parallel jobs** for non-merged batch concurrency.
- Click **Export npnp** to export all matched component IDs with the embedded Rust `npnp` integration.
- Merged `npnp` exports run sequentially and do not use the non-merged checkpoint flow.

> **Note:** The `nlbn` CLI must be installed and available in your system `PATH` to use the `nlbn` exporter. The `npnp` exporter is linked directly into the app.

---

### 4. Language — Switch between English and Chinese

<p align="center">
  <img src="public/imgs/Language.png" alt="Language Page" width="700" />
</p>

Click **English** or **Chinese** to switch the entire interface language. When Chinese is selected, the embedded **Source Han Sans** font is used for consistent rendering across all platforms. Your preference is saved and remembered.

---

### 5. About

<p align="center">
  <img src="public/imgs/About.png" alt="About Page" width="700" />
</p>

The **About** page shows app info, version, and the tech stack. Click on any technology chip to visit its website.

---

## Tech Stack

| | Technology | Role |
|---|---|---|
| <img src="src/assets/tauri.svg" width="20" /> | [Tauri](https://tauri.app) | App framework |
| <img src="src/assets/rust.svg" width="20" /> | [Rust](https://www.rust-lang.org) | Backend logic |
| <img src="src/assets/typescript.svg" width="20" /> | [TypeScript](https://www.typescriptlang.org) | Frontend logic |
| <img src="src/assets/vite.svg" width="20" /> | [Vite](https://vite.dev) | Build tooling |
|  | `nlbn` | External export CLI |
|  | `npnp` | Embedded export backend |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v18+)
- [Tauri CLI](https://tauri.app/start/)
- `nlbn` in your `PATH` if you want to use the `nlbn` export option
- Cargo access to resolve the published `npnp` `v0.1.6` crate from crates.io

### Development

```bash
npm install
npx tauri dev
```

### Build

```bash
npx tauri build
```

Installers are generated at:
- **Windows**: `src-tauri/target/release/bundle/msi/seex_0.3.3_x64_en-US.msi`
- **Windows**: `src-tauri/target/release/bundle/nsis/seex_0.3.3_x64-setup.exe`
- **macOS**: `.dmg` via GitHub Actions
- **Linux**: `.deb` / `.AppImage` via GitHub Actions

### Export Configuration

- Export settings are stored in `export_config.json` next to the built executable.
- Legacy `nlbn_config.txt` values are migrated on first load.
- `nlbn` and `npnp` each keep their own output path and batch settings.
- `npnp` parallel jobs apply only to non-merged batch exports.

## License

This project is licensed under [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/).

Free to use, share, and adapt for non-commercial purposes with attribution.
