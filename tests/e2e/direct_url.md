# URL import acceptance record

Build/tag: _pending_
Operator: _pending_

For every case record: input URL, project directory, result, proxy playback, transcript, `origin_url`, temporary-file cleanup, and exact error if unavailable.

| Case | Result | Evidence / limitation |
|---|---|---|
| Direct HTTP audio fixture | pending | Local HTTP server, full pipeline |
| Direct HTTP video fixture | pending | Local HTTP server, full pipeline and preview |
| YouTube public video | pending | yt-dlp + bundled Deno |
| Yandex Video public URL | pending | yt-dlp extractor |
| VK public video | pending | yt-dlp extractor |
| Unsupported URL | pending | Localized actionable error, no project |
| Login/private/DRM URL | pending | Explicit refusal/error, no bypass |
| Cancel during download | pending | Process stops, partial files removed |
| Non-ASCII portable/user path | pending | Download and import succeed |

Completion requires no leftover `.partial` project or downloader temp directory. Online services may be blocked by region, anti-bot protection or authentication; record that as an environment limitation, never as a successful import.
