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
- **Touch**, with finger-sized (44-point) buttons and rows:
  - Tap a block to edit it (the keyboard opens on the first tap, and the page shrinks so
    the keyboard never covers what you type). Drag anywhere to scroll.
  - **Move a block:** press and hold it, then drag — or drag the ⋮⋮ handle that stays next
    to the block you last tapped. Tap ⋮⋮ for turn-into, colours, duplicate and delete.
  - **Sidebar:** tap ☰ or swipe in from the left edge; swipe left or tap outside to close.
    Tap ⏵ to expand a page, use ··· / + on the open page, or press and hold any page for
    its menu (favorite, duplicate, move up/down, folder, trash).
  - **Back:** the ‹ button next to ☰ returns to the page you came from.
- **A folder for your notes** (workspace menu, bottom left):
  - **Chrome / Edge on a computer, Chrome on Android:** *Use a folder for my notes…* —
    pick any folder (for example inside Google Drive or OneDrive). Every page is saved
    there as a `.md` file, pictures in `_assets/`, sub-folders are kept, and changes made in
    the folder (synced from another device, edited in another app) show up within a few
    seconds. If the folder is empty, the notes already in the app are moved into it. The
    folder is remembered; if the browser asks again later, tap **Reconnect folder** at the
    top of the sidebar (edits made meanwhile are written to the folder first).
  - **Safari on iPhone / iPad (iOS 18.4+):** *Open a folder from Files…* loads all pages and
    pictures from a folder (iCloud Drive, Google Drive, OneDrive, On My iPhone…), keeping
    its sub-folders. **Safari does not let web apps save into a folder**, so your changes
    stay on the device; use *Export everything as .zip…* → **Save to Files** to write them
    back. See [Saving automatically on iPhone / iPad](#saving-automatically-on-iphone--ipad)
    for the options.
  - The notes are always also kept on the device, so the app works offline either way.
- **Other ways to move files** (through the Files app, which includes iCloud Drive, Google
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
also uses it to make the download smaller (optional). .

**Try it on your computer:** serve the folder over HTTP, e.g.
`python3 -m http.server 8080 --directory web/dist`, and open <http://localhost:8080>
(opening `index.html` straight from disk does not work: browsers only allow the offline
cache over `http://localhost` or `https://`).

### Saving your notes into a Files folder (iPhone / iPad)

Safari keeps the web app's notes on the device; to put them in a folder of the **Files** app
(iCloud Drive, On My iPhone, Google Drive, OneDrive…):

**All notes, the simple way**

1. Tap ☰ → the workspace name at the bottom of the sidebar → **Save to Files…**
   (if you opened a folder from Files and changed pages, a blue **Save to Files** button also
   appears at the top of the sidebar).
2. In the share sheet tap **Save to Files**, choose the place (for example *iCloud Drive*),
   tap **Save**. You get one `.zip` named like your folder.
3. In the Files app, tap the `.zip` once: it unpacks into a folder with all pages (`.md`),
   sub-folders and pictures. If an older copy of the folder is there, delete it first, or
   keep both.

**Just one page:** open the page → **···** (top right) → **Save this page to Files…** →
**Save to Files**. It saves that page as a single `.md` file.

**One tap, straight into the same folder (Shortcuts app)** — set up once per device:

1. Open **Shortcuts** → **+** → name it *Save Folio notes*.
2. Tap the **ⓘ** (details) → turn on **Show in Share Sheet**, and set it to receive **Files**.
3. Add the action **Extract Archive** (input: *Shortcut Input*).
4. Add the action **Save File**: turn **Ask Where to Save** off, choose the folder that
   *contains* your notes folder (e.g. *iCloud Drive*), and turn **Overwrite If File Exists** on.
5. In Folio: **Save to Files…** → in the share sheet pick **Save Folio notes** instead of *Save
   to Files*. The notes folder is replaced with the current version in one step.

(The Shortcut runs on the device and needs no internet. Apple's Shortcuts app is set up by
hand; Folio can't create it for you.)

### Saving automatically on iPhone / iPad

Safari gives web apps no way to write into a folder in the Files app, so on iPhone and
iPad the web app keeps your notes on the device and you save them back with *Export*.
These are the ways around that, from least to most work:

| Approach | Saves on its own? | Works offline? | Needs |
| --- | --- | --- | --- |
| **Export → Save to Files** (built in) | No, one tap + choose folder | Yes | Nothing |
| **Shortcuts app** bridge | Almost: one tap, then iOS runs a Shortcut that unzips into a fixed Files folder (iCloud Drive, Google Drive, OneDrive…) | Yes | A personal Shortcut on each device; the app would hand it the notes |
| **Google Drive / OneDrive / Dropbox connection** (built in, see [Cloud services](#cloud-services)) | Yes, in the background while the app is open | Edits queue offline and upload when online | Signing in to that service, and a free app registration with Google / Microsoft / Dropbox for this web address |
| **Native app** (same Rust code wrapped for iOS) | Yes, straight into any Files folder, like the desktop app | Yes | An Apple developer account ($99/year) to install through TestFlight or the App Store |

Notes:

- A native iPhone app **cannot be installed from a web page**: it must come from the App
  Store, TestFlight (an invite link; builds expire after 90 days), or be installed from a
  Mac with Xcode (a free Apple ID keeps it working for 7 days). Apple's EU "web
  distribution" is only open to developers with a large existing app.
- On **Android, Chrome** and on computers (**Chrome / Edge**), *Use a folder for my notes…*
  already saves straight into a folder, so no workaround is needed there.

#### What about iCloud Drive?

- **On a Mac it just works:** in the desktop app choose *Open or create a folder…* and pick
  a folder inside **iCloud Drive** (or use *Use a folder for my notes…* in Chrome / Edge).
  Every page is a `.md` file there, so it appears on your iPhone / iPad in the **Files** app
  under iCloud Drive.
- **On iPhone / iPad the web app cannot write into iCloud Drive.** Apple gives websites no
  access to iCloud Drive files. Its web toolkit for iCloud (CloudKit JS) stores data in an
  app's own private iCloud database — not as files you can see in iCloud Drive — and needs
  a paid Apple developer account.
- What works on iPhone / iPad today: *Open a folder from Files…* to **read** the iCloud Drive
  folder (for example the one the Mac app writes to), and *Export → Save to Files →
  iCloud Drive* to save back. The **Shortcuts** bridge in the table above can also save into
  iCloud Drive without asking each time, and a **native app** could save there
  automatically.

### Cloud services

Both versions can keep the notes in **Dropbox**, **OneDrive** or **Google Drive**, and they
use the same folder so the desktop app and the phone see the same pages:

| Service | Folder used |
| --- | --- |
| Dropbox | `Dropbox/Apps/Folio` |
| OneDrive | `OneDrive/Apps/Folio` |
| Google Drive | `My Drive/Folio Notes` |

**Desktop app:** install the service's own desktop app (Dropbox, OneDrive, Google Drive for
desktop), then in Folio open the workspace menu (bottom left) → *Use Dropbox / OneDrive /
Google Drive folder*. Folio finds the synced folder on your computer; if that folder is empty,
the notes you already have are copied into it. The service's app does the syncing.

**Web app (iPhone, iPad, any browser):** workspace menu → *Connect Dropbox / OneDrive /
Google Drive* → sign in. From then on every change is uploaded in the background, changes
made on other devices come down every ~15 seconds (and whenever you open the app), and edits
made offline are kept and uploaded when you're back online. The status is shown at the bottom
of the sidebar ("Dropbox · Synced", "Offline · 2 changes waiting"…). If the service folder is
empty, the notes on the device are uploaded into it. *Disconnect* keeps the notes on the device.

Each service only lets a web app sign people in once its owner has registered it (free).
Register once, put the keys in **`web/config.js`**, commit, and the build publishes them.
Use your web app's address as the redirect URI, exactly, with the final `/` —
e.g. `https://jgarcia241.github.io/folio/`.

1. **Dropbox** — <https://www.dropbox.com/developers/apps> → *Create app* → *Scoped access* →
   *App folder* → name it **Folio**. In *Settings* add the redirect URI; in *Permissions* tick
   `files.content.read` and `files.content.write` and press *Submit*. Copy the **App key**
   into `dropboxAppKey`.
2. **OneDrive** — <https://portal.azure.com> → *Microsoft Entra ID* → *App registrations* →
   *New registration*: name **Folio**, *Accounts in any organizational directory and personal
   Microsoft accounts*, redirect URI platform **Single-page application (SPA)** with your
   address. Under *API permissions* add Microsoft Graph → delegated `Files.ReadWrite.AppFolder`
   and `offline_access`. Copy the **Application (client) ID** into `onedriveClientId`.
3. **Google Drive** — <https://console.cloud.google.com> → create a project → *APIs & Services*
   → enable **Google Drive API** → *OAuth consent screen* (External; add yourself as a test
   user) → *Credentials* → *Create credentials* → *OAuth client ID* → **Web application**;
   add your address to *Authorized JavaScript origins* (`https://jgarcia241.github.io`) and to
   *Authorized redirect URIs* (`https://jgarcia241.github.io/folio/`). Copy the **Client ID**
   into `googleClientId`.

Limits that come from the services, not from Folio:

- **Google Drive:** Google gives apps that run only in the browser a sign-in that lasts about
  an hour. When it runs out, the sidebar shows *Sign in to Google Drive again*; your edits are
  kept on the device and uploaded after you sign in (one tap if you're still signed in to
  Google). Google also shows an "unverified app" notice while the app is in testing.
- **OneDrive:** Microsoft limits browser-only apps to about a day; then you sign in again the
  same way.
- **Dropbox** keeps you signed in until you disconnect.
- Two devices editing the **same page** at the same moment: the last one saved wins.

### Asking before closing

- **Desktop app:** if a change has not been written yet, closing asks **Save and close /
  Close without saving / Cancel**. After you have edited something it also asks *Close
  Folio?* even when everything is saved; turn that off with *Ask before closing* in the
  workspace menu (or the tick box in the question).
- **Web app:** pages save on every change. If you close the tab or app while a change is
  still being saved or uploaded, the browser asks *Leave this page?* first. (iPhone and iPad
  don't ask when you swipe an app away, but the app saves the moment it goes to the
  background.)

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
