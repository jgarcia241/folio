// Cloud services for the web app. Leave a key empty to hide that service.
// How to get each key (free, a few minutes each): see the README, "Cloud services".
// The address you register as "redirect URI" must be the web app's address,
// for example https://<your-user>.github.io/<repository>/ (with the final "/").
window.FOLIO_CLOUD = {
  // Dropbox: App console → Create app → "Scoped access" + "App folder", name it "Folio".
  dropboxAppKey: "",
  // OneDrive: Azure portal → App registrations → New, platform "Single-page application".
  onedriveClientId: "",
  // Google Drive: Google Cloud console → Credentials → OAuth client ID → "Web application".
  googleClientId: "",

  // Folder names (must match what the desktop app uses; usually leave as they are).
  dropboxFolderName: "Folio",
  onedriveFolderName: "Folio",
  googleFolderName: "Folio Notes",
};
