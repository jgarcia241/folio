// Folio web app: storage and sync.
//
// The Rust app keeps an in-memory copy of the notes and calls folioHost.put /
// folioHost.remove after every change. Every file is saved in this device's
// IndexedDB (so the app always works offline). Optionally the notes are also
// kept in ONE "backend", which this script keeps in step with the device copy:
//
//   * a folder on the device (File System Access: Chrome / Edge, Chrome on Android)
//   * Dropbox, OneDrive or Google Drive (sign in once; needs an app key in config.js)
//   * Safari on iPhone / iPad can also *read* a folder from Files (no writing).
//
// Changes made while offline (or signed out) are remembered and uploaded later.
(function () {
  "use strict";
  const DB_NAME = "folio", STORE = "files", META = "meta";
  const ROOT = "/My Notes/";
  const canWrite = typeof window.showDirectoryPicker === "function";
  const CFG = window.FOLIO_CLOUD || {};
  const wasm = () => window.folioWasm;
  const redirectUri = location.origin + location.pathname.replace(/index\.html$/, "");

  // ======================================================================= IndexedDB
  const dbReady = new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 2);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE);
      if (!db.objectStoreNames.contains(META)) db.createObjectStore(META);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  const pending = new Map();
  let timer = null;
  function schedule() { if (!timer) timer = setTimeout(flush, 40); }
  async function flush() {
    timer = null;
    if (pending.size === 0) return;
    const db = await dbReady;
    const items = [...pending];
    pending.clear();
    const st = db.transaction(STORE, "readwrite").objectStore(STORE);
    for (const [key, value] of items) {
      if (value === null) st.delete(key); else st.put(value, key);
    }
  }
  async function loadAll() {
    const db = await dbReady;
    return new Promise((resolve, reject) => {
      const out = [];
      const req = db.transaction(STORE, "readonly").objectStore(STORE).openCursor();
      req.onsuccess = () => {
        const c = req.result;
        if (c) { out.push([c.key, c.value]); c.continue(); } else resolve(out);
      };
      req.onerror = () => reject(req.error);
    });
  }
  async function stored(path) {
    const db = await dbReady;
    return new Promise((resolve) => {
      const req = db.transaction(STORE, "readonly").objectStore(STORE).get(path);
      req.onsuccess = () => resolve(req.result || null);
      req.onerror = () => resolve(null);
    });
  }
  async function meta(key, value) {
    const db = await dbReady;
    return new Promise((resolve, reject) => {
      const st = db.transaction(META, value === undefined ? "readonly" : "readwrite").objectStore(META);
      const req = value === undefined ? st.get(key) : value === null ? st.delete(key) : st.put(value, key);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }
  async function replaceStored(files) {
    await flush();
    const db = await dbReady;
    const existing = await loadAll();
    const st = db.transaction(STORE, "readwrite").objectStore(STORE);
    for (const [k] of existing) if (k.startsWith(ROOT)) st.delete(k);
    for (const [rel, data, mtime] of files) st.put({ data, mtime }, ROOT + rel);
  }

  // ======================================================================= helpers
  function wanted(rel) {
    // Pages anywhere, pictures in _assets/. Hidden files and other things are left alone.
    if (!rel || rel.split("/").some((p) => p.startsWith("."))) return false;
    return rel.toLowerCase().endsWith(".md") || rel.startsWith("_assets/");
  }
  class AuthNeeded extends Error {}
  class Offline extends Error {}
  async function http(url, opts) {
    let r;
    try { r = await fetch(url, opts); } catch (e) { throw new Offline(String(e)); }
    if (r.status === 401) throw new AuthNeeded("signed out");
    return r;
  }
  function b64url(bytes) {
    let s = "";
    for (const b of new Uint8Array(bytes)) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }
  function randomString(n) { return b64url(crypto.getRandomValues(new Uint8Array(n))); }
  async function pkcePair() {
    const verifier = randomString(48);
    const challenge = b64url(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier)));
    return { verifier, challenge };
  }
  function form(obj) { return new URLSearchParams(obj).toString(); }

  // ======================================================================= backends
  // Each backend: { kind, provider, name, pollMs, list() -> [[rel, stamp]], read(rel), write(rel, data) -> stamp, remove(rel) }

  // ---- a folder on the device (File System Access) ----
  function localBackend(handle) {
    async function dirFor(rel, create) {
      const parts = rel.split("/");
      let d = handle;
      for (const p of parts.slice(0, -1)) d = await d.getDirectoryHandle(p, { create });
      return [d, parts[parts.length - 1]];
    }
    async function walk(dir, prefix, out) {
      for await (const [name, h] of dir.entries()) {
        if (name.startsWith(".")) continue;
        const rel = prefix + name;
        if (h.kind === "directory") await walk(h, rel + "/", out);
        else if (wanted(rel)) out.push([rel, String((await h.getFile()).lastModified)]);
      }
      return out;
    }
    return {
      kind: "local", provider: "", name: handle.name, pollMs: 4000, handle,
      list: () => walk(handle, "", []),
      async read(rel) {
        const [d, name] = await dirFor(rel, false);
        return new Uint8Array(await (await (await d.getFileHandle(name)).getFile()).arrayBuffer());
      },
      async write(rel, data) {
        const [d, name] = await dirFor(rel, true);
        const fh = await d.getFileHandle(name, { create: true });
        const w = await fh.createWritable();
        await w.write(data);
        await w.close();
        return String((await fh.getFile()).lastModified);
      },
      async remove(rel) {
        try { const [d, name] = await dirFor(rel, false); await d.removeEntry(name); } catch (e) { return; }
        // Remove folders left empty.
        const parts = rel.split("/");
        for (let i = parts.length - 1; i > 0; i--) {
          try {
            let d = handle;
            for (const p of parts.slice(0, i - 1)) d = await d.getDirectoryHandle(p);
            await d.removeEntry(parts[i - 1]);
          } catch (e) { break; }
        }
      },
    };
  }

  // ---- sign-in tokens shared by the cloud backends ----
  async function tokenFor(acct) {
    if (acct.expires_at && Date.now() < acct.expires_at - 60000) return acct.access_token;
    if (!acct.refresh_token) throw new AuthNeeded("expired");
    const p = PROVIDERS[acct.kind];
    const r = await http(p.tokenUrl, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: form({ grant_type: "refresh_token", refresh_token: acct.refresh_token, client_id: p.clientId(), ...(p.scope ? { scope: p.scope } : {}) }),
    });
    if (!r.ok) throw new AuthNeeded("refresh failed: " + (await r.text()));
    const t = await r.json();
    acct.access_token = t.access_token;
    acct.expires_at = Date.now() + (t.expires_in || 3600) * 1000;
    if (t.refresh_token) acct.refresh_token = t.refresh_token;
    await meta("backend", acct);
    return acct.access_token;
  }

  // ---- Dropbox (notes in the app folder: Dropbox/Apps/<app name>) ----
  function dropboxBackend(acct) {
    const argHeader = (o) => JSON.stringify(o).replace(/[\u007f-￿]/g, (c) => "\\u" + ("000" + c.charCodeAt(0).toString(16)).slice(-4));
    async function rpc(path, body) {
      const r = await http("https://api.dropboxapi.com/2/" + path, {
        method: "POST",
        headers: { Authorization: "Bearer " + (await tokenFor(acct)), "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!r.ok) { const t = await r.text(); const e = new Error(t); e.status = r.status; throw e; }
      return r.json();
    }
    return {
      kind: "dropbox", provider: "Dropbox", name: "Apps/" + (CFG.dropboxFolderName || "Folio"), pollMs: 15000,
      async list() {
        const out = [];
        let res;
        try { res = await rpc("files/list_folder", { path: "", recursive: true }); }
        catch (e) { if (e.status === 409) return out; throw e; }
        for (;;) {
          for (const e of res.entries) if (e[".tag"] === "file") { const rel = e.path_display.slice(1); if (wanted(rel)) out.push([rel, e.rev]); }
          if (!res.has_more) break;
          res = await rpc("files/list_folder/continue", { cursor: res.cursor });
        }
        return out;
      },
      async read(rel) {
        const r = await http("https://content.dropboxapi.com/2/files/download", {
          method: "POST",
          headers: { Authorization: "Bearer " + (await tokenFor(acct)), "Dropbox-API-Arg": argHeader({ path: "/" + rel }) },
        });
        if (!r.ok) throw new Error("download " + r.status);
        return new Uint8Array(await r.arrayBuffer());
      },
      async write(rel, data) {
        const r = await http("https://content.dropboxapi.com/2/files/upload", {
          method: "POST",
          headers: {
            Authorization: "Bearer " + (await tokenFor(acct)),
            "Content-Type": "application/octet-stream",
            "Dropbox-API-Arg": argHeader({ path: "/" + rel, mode: "overwrite", mute: true }),
          },
          body: data,
        });
        if (!r.ok) throw new Error("upload " + r.status + " " + (await r.text()));
        return (await r.json()).rev;
      },
      async remove(rel) {
        try { await rpc("files/delete_v2", { path: "/" + rel }); } catch (e) { if (e.status !== 409) throw e; }
      },
    };
  }

  // ---- OneDrive (notes in the app folder: OneDrive/Apps/<app name>) ----
  function onedriveBackend(acct) {
    const base = "https://graph.microsoft.com/v1.0/me/drive/special/approot";
    const enc = (rel) => rel.split("/").map(encodeURIComponent).join("/");
    async function get(url) {
      const r = await http(url, { headers: { Authorization: "Bearer " + (await tokenFor(acct)) } });
      if (!r.ok) { const e = new Error("graph " + r.status); e.status = r.status; throw e; }
      return r.json();
    }
    async function children(url, prefix, out) {
      let next = url + "?$select=id,name,folder,file,cTag&$top=500";
      while (next) {
        const page = await get(next);
        for (const it of page.value) {
          const rel = prefix + it.name;
          if (it.folder) { if (!it.name.startsWith(".")) await children("https://graph.microsoft.com/v1.0/me/drive/items/" + it.id + "/children", rel + "/", out); }
          else if (wanted(rel)) out.push([rel, it.cTag]);
        }
        next = page["@odata.nextLink"];
      }
      return out;
    }
    return {
      kind: "onedrive", provider: "OneDrive", name: "Apps/" + (CFG.onedriveFolderName || "Folio"), pollMs: 15000,
      list: () => children(base + "/children", "", []),
      async read(rel) {
        const r = await http(base + ":/" + enc(rel) + ":/content", { headers: { Authorization: "Bearer " + (await tokenFor(acct)) } });
        if (!r.ok) throw new Error("download " + r.status);
        return new Uint8Array(await r.arrayBuffer());
      },
      async write(rel, data) {
        const r = await http(base + ":/" + enc(rel) + ":/content", {
          method: "PUT",
          headers: { Authorization: "Bearer " + (await tokenFor(acct)), "Content-Type": "application/octet-stream" },
          body: data,
        });
        if (!r.ok) throw new Error("upload " + r.status + " " + (await r.text()));
        return (await r.json()).cTag;
      },
      async remove(rel) {
        const r = await http(base + ":/" + enc(rel), { method: "DELETE", headers: { Authorization: "Bearer " + (await tokenFor(acct)) } });
        if (!r.ok && r.status !== 404) throw new Error("delete " + r.status);
      },
    };
  }

  // ---- Google Drive (notes in My Drive/Folio Notes) ----
  function googleBackend(acct) {
    const API = "https://www.googleapis.com/drive/v3/files";
    const UPLOAD = "https://www.googleapis.com/upload/drive/v3/files";
    const FOLDER = "application/vnd.google-apps.folder";
    const rootName = CFG.googleFolderName || "Folio Notes";
    const ids = new Map(); // rel path (files) or "dir:<path>" (folders) -> id
    async function req(url, opts = {}) {
      const token = await tokenFor(acct);
      const r = await http(url, { ...opts, headers: { ...(opts.headers || {}), Authorization: "Bearer " + token } });
      if (!r.ok) { const e = new Error("drive " + r.status + " " + (await r.text())); e.status = r.status; throw e; }
      return r;
    }
    const q = (s) => s.replace(/\\/g, "\\\\").replace(/'/g, "\\'");
    async function find(name, parent, folder) {
      const query = `name='${q(name)}' and '${parent}' in parents and trashed=false` + (folder ? ` and mimeType='${FOLDER}'` : "");
      const res = await (await req(API + "?q=" + encodeURIComponent(query) + "&fields=files(id)&pageSize=1")).json();
      return res.files[0] ? res.files[0].id : null;
    }
    async function makeFolder(name, parent) {
      const r = await req(API + "?fields=id", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ name, mimeType: FOLDER, parents: [parent] }) });
      return (await r.json()).id;
    }
    async function rootId() {
      if (ids.has("dir:")) return ids.get("dir:");
      const id = (await find(rootName, "root", true)) || (await makeFolder(rootName, "root"));
      ids.set("dir:", id);
      return id;
    }
    async function dirId(path) {
      if (!path) return rootId();
      if (ids.has("dir:" + path)) return ids.get("dir:" + path);
      const parts = path.split("/");
      const parent = await dirId(parts.slice(0, -1).join("/"));
      const name = parts[parts.length - 1];
      const id = (await find(name, parent, true)) || (await makeFolder(name, parent));
      ids.set("dir:" + path, id);
      return id;
    }
    async function walk(folderId, prefix, out) {
      let token = "";
      do {
        const query = `'${folderId}' in parents and trashed=false`;
        const url = API + "?q=" + encodeURIComponent(query) + "&fields=nextPageToken,files(id,name,mimeType,md5Checksum,modifiedTime)&pageSize=1000" + (token ? "&pageToken=" + token : "");
        const res = await (await req(url)).json();
        for (const f of res.files) {
          const rel = prefix + f.name;
          if (f.mimeType === FOLDER) { ids.set("dir:" + rel, f.id); await walk(f.id, rel + "/", out); }
          else if (wanted(rel)) { ids.set(rel, f.id); out.push([rel, f.md5Checksum || f.modifiedTime]); }
        }
        token = res.nextPageToken || "";
      } while (token);
      return out;
    }
    return {
      kind: "google", provider: "Google Drive", name: rootName, pollMs: 15000,
      async list() { ids.clear(); return walk(await rootId(), "", []); },
      async read(rel) {
        const id = ids.get(rel);
        if (!id) throw new Error("not found");
        return new Uint8Array(await (await req(API + "/" + id + "?alt=media")).arrayBuffer());
      },
      async write(rel, data) {
        let id = ids.get(rel);
        if (id) {
          const r = await req(UPLOAD + "/" + id + "?uploadType=media&fields=md5Checksum,modifiedTime", { method: "PATCH", body: data });
          const j = await r.json();
          return j.md5Checksum || j.modifiedTime;
        }
        const parts = rel.split("/");
        const parent = await dirId(parts.slice(0, -1).join("/"));
        const boundary = "folio" + randomString(12);
        const head = `--${boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n${JSON.stringify({ name: parts[parts.length - 1], parents: [parent] })}\r\n--${boundary}\r\nContent-Type: application/octet-stream\r\n\r\n`;
        const body = new Blob([head, data, `\r\n--${boundary}--`]);
        const r = await req(UPLOAD + "?uploadType=multipart&fields=id,md5Checksum,modifiedTime", { method: "POST", headers: { "Content-Type": "multipart/related; boundary=" + boundary }, body });
        const j = await r.json();
        ids.set(rel, j.id);
        return j.md5Checksum || j.modifiedTime;
      },
      async remove(rel) {
        const id = ids.get(rel);
        if (!id) return;
        await req(API + "/" + id, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ trashed: true }) });
        ids.delete(rel);
      },
    };
  }

  // ---- sign-in settings for each service ----
  const PROVIDERS = {
    dropbox: {
      label: "Dropbox",
      clientId: () => CFG.dropboxAppKey || "",
      authUrl: (c, state, challenge) => "https://www.dropbox.com/oauth2/authorize?" + form({ client_id: c, response_type: "code", redirect_uri: redirectUri, code_challenge: challenge, code_challenge_method: "S256", token_access_type: "offline", state }),
      tokenUrl: "https://api.dropboxapi.com/oauth2/token",
      make: dropboxBackend,
    },
    onedrive: {
      label: "OneDrive",
      clientId: () => CFG.onedriveClientId || "",
      scope: "Files.ReadWrite.AppFolder offline_access",
      authUrl: (c, state, challenge) => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?" + form({ client_id: c, response_type: "code", redirect_uri: redirectUri, scope: "Files.ReadWrite.AppFolder offline_access", code_challenge: challenge, code_challenge_method: "S256", state }),
      tokenUrl: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
      make: onedriveBackend,
    },
    google: {
      label: "Google Drive",
      clientId: () => CFG.googleClientId || "",
      // Google gives browser-only apps short sign-ins (about an hour) without a server.
      authUrl: (c, state) => "https://accounts.google.com/o/oauth2/v2/auth?" + form({ client_id: c, response_type: "token", redirect_uri: redirectUri, scope: "https://www.googleapis.com/auth/drive", include_granted_scopes: "true", state }),
      make: googleBackend,
    },
  };

  // ======================================================================= sync engine
  let backend = null;
  let mode = 0;              // 0 none, 1 linked, 2 read from Files (Safari), 3 needs reconnect / sign-in
  let folderName = "";
  let known = new Map();     // rel -> stamp of the backend's copy
  let unsynced = new Set();  // rel paths changed on this device but not in the backend yet
  let busy = 0;
  let syncing = false;
  let lastError = "";
  let queue = Promise.resolve();

  // Pages changed since a folder was read from Files (Safari can't save them back by itself).
  const changedSinceRead = new Set();
  function statusText() {
    if (!backend && mode === 2) return changedSinceRead.size ? `${changedSinceRead.size} change${changedSinceRead.size === 1 ? "" : "s"} not saved to Files` : "";
    if (!backend) return "";
    if (mode === 3) return "Sign in again";
    if (!navigator.onLine && backend.kind !== "local") return unsynced.size ? `Offline · ${unsynced.size} change${unsynced.size === 1 ? "" : "s"} waiting` : "Offline";
    if (busy > 0 || syncing) return "Syncing…";
    if (unsynced.size) return `${unsynced.size} change${unsynced.size === 1 ? "" : "s"} waiting`;
    return lastError ? "Sync problem" : "Synced";
  }
  function state() {
    if (!wasm()) return;
    wasm().folio_folder_state(backend ? backend.name : folderName, mode, canWrite, backend ? backend.provider : "", statusText());
  }
  async function saveKnown() {
    await meta("known", Object.fromEntries(known));
    await meta("unsynced", [...unsynced]);
  }
  function enqueue(fn) {
    busy++;
    state();
    queue = queue.then(fn).catch((e) => console.warn("sync:", e)).finally(() => { busy--; state(); });
    return queue;
  }
  function failed(e, rel) {
    if (rel) unsynced.add(rel);
    if (e instanceof AuthNeeded) { mode = 3; }
    else if (!(e instanceof Offline)) { lastError = String(e); console.warn("sync:", e); }
    saveKnown();
    state();
  }
  async function pushOne(rel) {
    const v = await stored(ROOT + rel);
    if (v) known.set(rel, await backend.write(rel, v.data));
    else { await backend.remove(rel); known.delete(rel); }
    unsynced.delete(rel);
  }
  async function pushUnsynced() {
    for (const rel of [...unsynced]) await pushOne(rel);
    await saveKnown();
  }
  /// Bring changes made in the backend into the app.
  async function pull() {
    const now = await backend.list();
    const seen = new Set();
    for (const [rel, stamp] of now) {
      seen.add(rel);
      if (unsynced.has(rel)) continue; // this device's newer change wins
      if (known.get(rel) !== stamp) {
        const data = await backend.read(rel);
        known.set(rel, stamp);
        wasm().folio_external_put(ROOT + rel, data, Date.now());
        pending.set(ROOT + rel, { data, mtime: Date.now() });
      }
    }
    for (const rel of [...known.keys()]) {
      if (!seen.has(rel) && !unsynced.has(rel)) {
        known.delete(rel);
        wasm().folio_external_remove(ROOT + rel);
        pending.set(ROOT + rel, null);
      }
    }
    schedule();
    await saveKnown();
  }
  async function syncNow() {
    if (!backend || mode !== 1 || syncing || busy > 0) return;
    if (backend.kind !== "local" && !navigator.onLine) { state(); return; }
    syncing = true;
    state();
    try {
      await pushUnsynced();
      await pull();
      lastError = "";
    } catch (e) { failed(e); }
    finally { syncing = false; state(); }
  }
  /// Hand a whole set of files to the app (it re-reads the notes).
  async function loadIntoApp(files) {
    await replaceStored(files);
    pending.clear();
    wasm().folio_folder_begin();
    for (const [rel, data, mtime] of files) wasm().folio_folder_file(rel, data, mtime || 0);
    wasm().folio_folder_end();
  }
  /// First connection to a backend: use its notes, or move this device's notes into it if it's empty.
  async function attach(b) {
    backend = b;
    mode = 1;
    lastError = "";
    syncing = true;
    state();
    try {
      let listing = await b.list();
      if (!listing.some(([rel]) => rel.toLowerCase().endsWith(".md"))) {
        for (const [k, v] of await loadAll()) if (k.startsWith(ROOT)) await b.write(k.slice(ROOT.length), v.data);
        listing = await b.list();
      }
      const files = [];
      for (const [rel, stamp] of listing) files.push([rel, await b.read(rel), Date.now(), stamp]);
      known = new Map(files.map(([rel, , , stamp]) => [rel, stamp]));
      unsynced = new Set();
      await loadIntoApp(files.map(([rel, data, mtime]) => [rel, data, mtime]));
      await saveKnown();
    } catch (e) { failed(e); }
    finally { syncing = false; state(); }
  }
  async function restoreBackend(saved) {
    if (saved.kind === "local") {
      const handle = saved.handle;
      folderName = handle.name;
      let perm = "prompt";
      try { perm = await handle.queryPermission({ mode: "readwrite" }); } catch (e) {}
      backend = localBackend(handle);
      if (perm !== "granted") { mode = 3; state(); return; }
    } else {
      const p = PROVIDERS[saved.kind];
      if (!p) return;
      backend = p.make(saved);
    }
    mode = 1;
    known = new Map(Object.entries((await meta("known").catch(() => null)) || {}));
    unsynced = new Set((await meta("unsynced").catch(() => null)) || []);
    state();
    await syncNow();
  }

  setInterval(() => { if (backend && !document.hidden) { if (Date.now() - (window.__folioLastSync || 0) >= backend.pollMs) { window.__folioLastSync = Date.now(); syncNow(); } } }, 2000);
  document.addEventListener("visibilitychange", () => { if (document.hidden) flush(); else syncNow(); });
  window.addEventListener("online", () => { state(); syncNow(); });
  window.addEventListener("offline", state);
  window.addEventListener("pagehide", flush);
  // Ask before leaving when something isn't saved yet (in the app, on the device or in the cloud).
  window.addEventListener("beforeunload", (e) => {
    const appUnsaved = wasm() && wasm().folio_has_unsaved && wasm().folio_has_unsaved();
    if (appUnsaved || pending.size > 0 || busy > 0 || (backend && unsynced.size > 0 && mode === 1 && navigator.onLine)) {
      flush();
      e.preventDefault();
      e.returnValue = "";
    }
  });

  // ======================================================================= sign-in redirects
  /// Called once at start-up: finish a sign-in that just came back to the app.
  async function finishSignIn() {
    const raw = localStorage.getItem("folio_oauth");
    if (!raw) return false;
    const params = new URLSearchParams(location.search);
    const hash = new URLSearchParams(location.hash.replace(/^#/, ""));
    const got = params.get("state") || hash.get("state");
    if (!got) return false;
    const st = JSON.parse(raw);
    localStorage.removeItem("folio_oauth");
    try { history.replaceState(null, "", location.pathname); } catch (e) { /* keep the address */ }
    if (got !== st.state) { console.warn("sign-in: state mismatch"); return false; }
    const err = params.get("error") || hash.get("error");
    if (err) { console.warn("sign-in failed:", err, params.get("error_description") || ""); return false; }
    const p = PROVIDERS[st.kind];
    let acct;
    if (st.kind === "google") {
      acct = { kind: "google", access_token: hash.get("access_token"), expires_at: Date.now() + Number(hash.get("expires_in") || 3600) * 1000 };
    } else {
      const r = await fetch(p.tokenUrl, {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: form({ grant_type: "authorization_code", code: params.get("code"), client_id: p.clientId(), redirect_uri: redirectUri, code_verifier: st.verifier, ...(p.scope ? { scope: p.scope } : {}) }),
      });
      if (!r.ok) { console.warn("sign-in token:", await r.text()); return false; }
      const t = await r.json();
      acct = { kind: st.kind, access_token: t.access_token, refresh_token: t.refresh_token || "", expires_at: Date.now() + (t.expires_in || 3600) * 1000 };
    }
    // Keep the device's pending changes if this is a re-sign-in to the same service.
    const prev = await meta("backend").catch(() => null);
    acct.reconnect = !!(prev && prev.kind === st.kind);
    await meta("backend", acct);
    return acct;
  }

  async function connectCloud(kind) {
    const p = PROVIDERS[kind];
    if (!p || !p.clientId()) { alert(`${p ? p.label : kind} is not set up yet. Add its app key to config.js (see the README).`); return; }
    await flush();
    const state_ = randomString(16);
    const { verifier, challenge } = await pkcePair();
    localStorage.setItem("folio_oauth", JSON.stringify({ kind, state: state_, verifier }));
    location.assign(p.authUrl(p.clientId(), state_, challenge));
  }

  // ======================================================================= the host the app talks to
  window.folioHost = {
    loadAll,
    async start() {
      wasm().folio_cloud_config(!!CFG.dropboxAppKey, !!CFG.onedriveClientId, !!CFG.googleClientId);
      let signedIn = false;
      try { signedIn = await finishSignIn(); } catch (e) { console.warn("sign-in:", e); }
      let saved = await meta("backend").catch(() => null);
      if (!saved) {
        // Folders linked by an earlier version were stored under "folder".
        const old = await meta("folder").catch(() => null);
        if (old) { saved = { kind: "local", handle: old }; await meta("backend", saved); await meta("folder", null); }
      }
      if (signedIn && !signedIn.reconnect) {
        await attach(PROVIDERS[signedIn.kind].make(signedIn));
      } else if (saved) {
        await restoreBackend(saved);
      } else {
        state();
      }
    },
    put(path, data, mtime) {
      const copy = new Uint8Array(data); // `data` is a view into the app's memory
      pending.set(path, { data: copy, mtime });
      schedule();
      if (mode === 2 && path.startsWith(ROOT)) { changedSinceRead.add(path); state(); }
      if (backend && path.startsWith(ROOT)) {
        const rel = path.slice(ROOT.length);
        unsynced.add(rel);
        if (mode === 1 && (backend.kind === "local" || navigator.onLine)) {
          enqueue(async () => { await flush(); try { await pushOne(rel); await saveKnown(); } catch (e) { failed(e, rel); } });
        } else { saveKnown(); state(); }
      }
    },
    remove(path) {
      pending.set(path, null);
      schedule();
      if (mode === 2 && path.startsWith(ROOT)) { changedSinceRead.add(path); state(); }
      if (backend && path.startsWith(ROOT)) {
        const rel = path.slice(ROOT.length);
        unsynced.add(rel);
        if (mode === 1 && (backend.kind === "local" || navigator.onLine)) {
          enqueue(async () => { await flush(); try { await pushOne(rel); await saveKnown(); } catch (e) { failed(e, rel); } });
        } else { saveKnown(); state(); }
      }
    },
    syncNow,
    connectCloud,

    // A folder for the notes.
    async pickFolder() {
      try {
        if (canWrite) {
          const handle = await window.showDirectoryPicker({ id: "folio-notes", mode: "readwrite" });
          await meta("backend", { kind: "local", handle });
          folderName = handle.name;
          await attach(localBackend(handle));
        } else {
          // Safari (iPhone/iPad): read a folder from Files.
          const input = document.createElement("input");
          input.type = "file";
          input.webkitdirectory = true;
          input.multiple = true;
          input.style.display = "none";
          document.body.appendChild(input);
          input.addEventListener("change", async () => {
            const files = [];
            let top = "";
            for (const f of input.files) {
              const parts = (f.webkitRelativePath || f.name).split("/");
              top = top || (parts.length > 1 ? parts[0] : "");
              const rel = parts.length > 1 ? parts.slice(1).join("/") : parts[0];
              if (wanted(rel)) files.push([rel, new Uint8Array(await f.arrayBuffer()), f.lastModified]);
            }
            input.remove();
            backend = null;
            folderName = top || "Files";
            mode = 2;
            await loadIntoApp(files);
            state();
          });
          input.click();
        }
      } catch (e) {
        if (!(e && e.name === "AbortError")) console.warn("pick folder:", e);
      }
    },
    async reconnectFolder() {
      const saved = await meta("backend").catch(() => null);
      if (!saved) return;
      if (saved.kind !== "local") { connectCloud(saved.kind); return; }
      const perm = await saved.handle.requestPermission({ mode: "readwrite" });
      if (perm !== "granted") return;
      backend = localBackend(saved.handle);
      mode = 1;
      known = new Map(Object.entries((await meta("known").catch(() => null)) || {}));
      state();
      await syncNow();
    },
    async forgetFolder() {
      await meta("backend", null).catch(() => {});
      await meta("known", null).catch(() => {});
      await meta("unsynced", null).catch(() => {});
      backend = null; folderName = ""; mode = 0; known = new Map(); unsynced = new Set();
      state();
    },

    // Choose files (Files app, Photos, iCloud Drive, Google Drive, OneDrive...).
    pick(id, accept, multiple) {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = accept;
      input.multiple = multiple;
      input.style.display = "none";
      document.body.appendChild(input);
      input.addEventListener("change", async () => {
        for (const f of input.files) wasm().folio_picked(id, f.name, new Uint8Array(await f.arrayBuffer()));
        input.remove();
      });
      input.click();
    },

    // Export: the share sheet on iPhone/iPad ("Save to Files"), a download elsewhere.
    async share(name, data, mime) {
      const blob = new Blob([new Uint8Array(data)], { type: mime });
      const file = new File([blob], name, { type: mime });
      const saved = () => { if (name.endsWith(".zip")) { changedSinceRead.clear(); state(); } };
      if (navigator.canShare && navigator.canShare({ files: [file] })) {
        try { await navigator.share({ files: [file], title: name }); saved(); return; }
        catch (e) { if (e && e.name === "AbortError") return; }
      }
      saved();
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = name;
      document.body.appendChild(a);
      a.click();
      setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 2000);
    },

    openUrl(url) { window.open(url, "_blank", "noopener"); },

    // For tests: plug in a backend directly.
    __attachForTest: (b) => attach(b),
  };
})();
