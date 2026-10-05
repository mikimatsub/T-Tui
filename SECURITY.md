# Security and privacy

Do not post tokens, browser storage, copied cURL requests, profile photos or message content in an issue. Use GitHub's private vulnerability reporting feature when enabled on the public repository. Otherwise contact the maintainer privately before disclosing exploit details; no private contact address is asserted here.

This is an unofficial client of a private service. It cannot guarantee future API compatibility, avoid account restrictions or replace Tinder's verification and reporting workflows. Automated tests never access a live account. Browser extraction is experimental and Chromium storage formats are not an authentication contract.

On Windows, saved auth tokens, refresh tokens and device IDs are protected by current-user DPAPI. This does not protect against software running as that user or an administrator. Drafts, names, pins and settings remain readable JSON. Legacy plaintext credentials are protected on the next successful save. A protected config cannot be copied to another Windows identity or Linux; import a session separately there. Unix config files are created with mode `0600` and are not encrypted.

Photo files remain local after sign-out. The cache prunes entries older than seven days and limits itself to 256 MiB, on startup and after downloads. Close T-TUI and run `ttui --clear-cache` to remove managed cached photos. This is file deletion, not secure erasure. The offline demo has separate configuration, and `TTUI_DATA_DIR` supports isolated storage.

Sign-out must successfully replace the saved configuration before reporting success. A write failure leaves an actionable dialog and preserves the session for retry. Mutations are never automatically replayed. HTTP rate limits share a cooldown; ambiguous acknowledgements remain failures requiring review in Tinder.

Supported updates target the current release candidate and, after release, the latest stable version. This hobby project offers no response-time guarantee or paid security service.
