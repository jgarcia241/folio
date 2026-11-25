# Folio

A fast, local-first notes app written in Rust. Pages, task lists and pictures are
saved as **plain Markdown files in a folder you choose**, so you can keep that folder
in Google Drive, OneDrive, Dropbox, iCloud Drive or Syncthing and share it between
computers. Folio never connects to the internet.

It runs as a **desktop app** (macOS, Windows, Linux) and as an **installable offline web
app (PWA) for iPhone and iPad** built from the same Rust code — see
[iPhone and iPad app](#iphone-and-ipad-app-offline-pwa).

## Features

- **Pages and sub-pages** in a sidebar tree, with favorites, search (⌘K / Ctrl+K) and trash.
- **Block editor**: text, three heading sizes, bulleted / numbered / to-do / toggle lists,
  quotes, callouts, code, dividers, tables, pictures and link previews.
  - Type `/` for the block menu, or Markdown shortcuts (`# `, `- `, `[] `, `1. `, `> `, ```` ``` ````, `---`).
  - Inline `**bold**`, `*italic*`, `` `code` ``, `~~strike~~` and `[links](https://…)`.
  - **Drag blocks** by the ⋮⋮ handle (nested list items move with their parent); click
    the handle for turn-into, background colour, duplicate and delete.
  - **Paste a link** and choose *Mention*, *URL* or *Preview* (a card with the site and
    address; you can type your own title and note, and nothing is downloaded).
  - **Pictures**: type `/image`, or drop picture files on the page. Drag the right edge
    to resize; add a caption.
- **Covers**: a gallery of drawn patterns, colours and gradients, or upload your own picture.
- **Databases** (task lists): typed columns (text, number, checkbox, date, select,
  multi-select), **Table** and **Board** views, **filters**, **sorting** and quick search.
  Each row is its own page and file.
- **Folders (optional)**: turn on *Keep sub-pages in a folder* for a page and its
  sub-pages are stored in a folder with the same name as the page's `.md` file.
- Light and dark mode; changes made by a sync tool on another computer show up within a couple of seconds.

## How your notes are stored

```
My Notes/                     ← the folder you pick (put it inside Drive / OneDrive…)
├── Getting Started.md
├── To-do.md                  ← a database page (column definitions in its header)
├── Book a dentist appointment.md   ← a database row (values in its header)
├── Projects.md               ← a page with "Keep sub-pages in a folder" on
├── Projects/
│   ├── Alpha.md
│   └── Beta.md
└── _assets/                  ← pictures used in pages and covers
    └── sample-hills.png
```

Every file starts with a small header (`id`, `title`, `parent`, `icon`, …) followed by
normal Markdown, so you can read and edit the files in any text editor. Plain `.md` files
you drop into the folder appear as pages. Saving writes to a temporary file first and
then renames it, so a sync tool never picks up a half-written page.

App settings (the last folder you opened, dark mode) are kept outside the notes folder:
`~/Library/Application Support/Folio/settings.txt` on macOS,
`%APPDATA%\Folio\settings.txt` on Windows, `~/.config/folio/settings.txt` on Linux.

## Building

You need the Rust toolchain (1.85 or newer, edition 2024).

1. Install Rust from <https://rustup.rs> (one command; it installs `cargo`).
2. On **Linux** only, install the system libraries used for windows and file dialogs, e.g. on Debian/Ubuntu:
   ```sh
   sudo apt install build-essential pkg-config libgtk-3-dev libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
   ```
   On macOS you need the Xcode command line tools (`xcode-select --install`); on Windows,
   the "Desktop development with C++" workload of Visual Studio Build Tools.
3. Build and run:
   ```sh
   cargo build --release
   ./target/release/folio          # Windows: target\release\folio.exe
   ```
   or simply `cargo run --release`.

The first build downloads and compiles the libraries (a few minutes); later builds are fast.

### Tests

```sh
cargo test
```

The tests cover the Markdown file format round trip, saving and reloading a workspace,
folder pages (create, rename, turn off), database filters, block drag-and-drop moves and
the `.zip` import/export format.

### Screenshot mode

`folio --screenshot out.png` opens the app with a temporary sample workspace (your notes are
not touched), draws about 120 frames, saves a picture of the window and exits. It is used
for automated visual checks.

## iPhone and iPad app (offline PWA)

The same app compiled to WebAssembly. Once installed it opens and works with **no
internet connection**: the app itself is cached on the device by a service worker, and
your notes are saved on the device (in the browser's IndexedDB storage), so nothing
leaves the phone.

- **Phone layout** below 760 points wide: the sidebar slides over the page (tap ☰),
  tighter margins, compact toolbars. **iPad** gets the full layout with the sidebar.
- **Touch**: tap a block to edit it (the keyboard opens on the first tap), drag
  anywhere to scroll, and use the ⋮⋮ handle next to the block you are editing to move it
  or open its menu.
- **Reading and writing files.** iOS does not let web apps open a folder directly, so
  files go in and out through the Files app (which includes iCloud Drive, Google
  Drive and OneDrive):
  - *Import notes or pictures…* (workspace menu, bottom left) reads `.md` pages,
    pictures, or a **`.zip` of a whole notes folder** (for example the desktop app's folder).
  - *Export everything as .zip…* opens the share sheet — choose **Save to Files** to put
    the folder (pages, sub-folders and pictures) wherever you like. Import that zip on the
    desktop app's side by unzipping it into your notes folder.
  - Pictures for image blocks and covers come from Photos or Files.

### Build the web app

You need the Rust toolchain (see [Building](#building)). Then, from this folder:

```sh
./web/build.sh
```

The script adds the `wasm32-unknown-unknown` target, installs the matching
`wasm-bindgen` tool the first time (version 0.2.130), compiles the app with the size-optimised
`web` profile and writes the finished site to **`web/dist/`**:

```
web/dist/
├── index.html             page that hosts the app, storage and file picking
├── folio.js, folio_bg.wasm   the app (≈ 2.7 MB compressed)
├── sw.js                  offline cache (renewed on every build)
├── manifest.webmanifest   name, colours and icons for "Add to Home Screen"
└── icons/
```

If [Binaryen](https://github.com/WebAssembly/binaryen)'s `wasm-opt` is installed the script
also uses it to make the download smaller (optional).

**Try it on your computer:** serve the folder over HTTP, e.g.
`python3 -m http.server 8080 --directory web/dist`, and open <http://localhost:8080>
(opening `index.html` straight from disk does not work: browsers only allow the offline
cache over `http://localhost` or `https://`).

### Put it online and install it on iPhone / iPad

The app is a set of static files, so any HTTPS static host works (GitHub Pages, Netlify,
Cloudflare Pages, your own server). It only needs the internet for the **first visit**.

**With GitHub Pages (automatic):** this repository includes
`.github/workflows/web.yml`. Push to `main`, then in the repository go to
*Settings → Pages → Source* and choose **GitHub Actions**. Every push then builds
`web/dist` and publishes it at `https://<your-user>.github.io/<repository>/`.

**By hand:** run `./web/build.sh` and upload the *contents* of `web/dist/` to your host.

**Seeing the README instead of the app?** GitHub Pages is set to *Deploy from a
branch*, which publishes the repository files (and renders `README.md`) instead of the
built app. Fix it once:

1. Open `https://github.com/<your-user>/<repository>/settings/pages`.
2. Under **Build and deployment → Source**, choose **GitHub Actions** (not *Deploy from a branch*).
3. Go to the **Actions** tab → **Web app (PWA)** → **Run workflow** (or push a commit) and wait
   for both jobs (*build* and *deploy*) to finish with a green check.
4. Open `https://<your-user>.github.io/<repository>/` — you should see the app, not the README.
   If you already added the README page to your Home Screen, delete that icon and add it
   again from the app's page.

The workflow checks this setting first and stops with the same instructions if it is wrong.

**Install:** open the address in **Safari** on the iPhone or iPad → tap **Share** →
**Add to Home Screen**. Open Folio from the new icon once while online so it can cache
itself; after that it works in airplane mode. A new version is picked up the next time
you open the app with a connection.

> Notes stay on the device the app was installed on. Use *Export everything as .zip…*
> regularly (or before deleting the app) as a backup, and *Import* to bring notes over
> from another device.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| ⌘K / ⌘P (Ctrl on Windows/Linux) | Search pages |
| ⌘N | New page |
| ⌘S | Save now (pages also save automatically) |
| ⌘\ | Show / hide the sidebar |
| `/` | Block menu |
| Tab / Shift+Tab | Indent / outdent list items |
| Enter / Backspace | New block / merge with the block above |
| ⌘-click a link | Open it in your browser |

## Project layout

| File | What it does |
| --- | --- |
| `src/main.rs` | Desktop entry point |
| `src/lib.rs` | Desktop window setup (`run_native`) and the web entry points (`folio_start`, …) |
| `src/app.rs` | App state, the frame loop, phone/touch layout, screenshot mode |
| `src/vfs.rs` | File access: the real disk on desktop, an IndexedDB-backed copy on the web |
| `src/platform.rs` | File pickers, opening links, saving exports, keyboard helper (desktop vs web) |
| `src/zip.rs` | Reading and writing `.zip` files for import and export |
| `src/model.rs` | Pages, blocks, databases and the Markdown file format |
| `src/store.rs` | The notes folder: scanning, saving, folders, pictures, sync polling |
| `src/editor.rs` | Sidebar, page view, block editor, menus, search, trash |
| `src/db.rs` | Database table and board views, filters and sorting |
| `src/covers.rs` | Cover patterns, colours and gradients |
| `src/theme.rs` | Colours and fonts |
| `web/` | Web app shell: `index.html`, `sw.js`, `manifest.webmanifest`, icons, `build.sh` |
| `assets/fonts/` | Roboto (Apache License 2.0), used on the web and when system fonts are missing |
| `examples/make_icons.rs` | Draws the app icons (`cargo run --example make_icons`) |

Built with [eframe/egui](https://github.com/emilk/egui). Roboto font © Google, Apache License 2.0.
