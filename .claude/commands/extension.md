# /extension

When working on `extension/` files, always keep the runtime boundaries in mind:

1. `service-worker.js` runs in Manifest V3 service worker context. It has no DOM access and can be suspended between async steps.
2. `content-script.js` runs in the ISOLATED world. It has `chrome.*` APIs but does not patch page-owned `fetch` or `XMLHttpRequest`.
3. `content-script-main.js` runs in the MAIN world. It can observe page-owned `fetch` and `XMLHttpRequest`, but it has no direct `chrome.*` access.
4. `blade-ui.js` runs in the ISOLATED world at `document_idle`.
5. `prompt.js` runs inside `prompt.html`.

Common failure modes:

- Service worker suspension mid-async: cancel browser downloads synchronously before awaiting anything.
- Extension reload invalidating old tab contexts: wrap runtime messaging in `isExtensionAlive()` where stale scripts can linger.
- Prompt opening as a full tab instead of a popup: known Chrome limitation from MV3 service workers; do not assume popup behavior is reliable.
- Duplicate injection: `all_frames` is `false` on all content scripts; do not flip it casually.

Native bridge payload shape:

```js
{
  type: 'queue_download',
  url,
  filename,
  size,
  quality,
  source,
  pageUrl,
  customHeaders
}
```
