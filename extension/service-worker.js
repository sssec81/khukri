const HOST_NAME = 'com.khukri.host';
const STREAM_PATTERNS = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i];

let nativePort = null;
let lastDisconnectTime = 0;
const RECONNECT_BACKOFF_MS = 1000; // Wait 1 second before retrying after disconnect
const recentRequests = new Map();
const RECENT_REQUESTS_MAX = 500;
const RECENT_REQUESTS_TTL_MS = 4000;
const latestStreamByTab = new Map();

function isTargetStream(url) {
    return STREAM_PATTERNS.some((pattern) => pattern.test(url || ''));
}

function normalizeFilename(filename, fallbackUrl) {
    const source = filename || (fallbackUrl ? fallbackUrl.split('?')[0].split('/').pop() : '') || 'download.bin';
    return source.replace(/[<>:"/\\|?*]/g, '_') || 'download.bin';
}

function buildCustomHeaders({ referer, pageUrl }) {
    const headers = {};
    const ua = self.navigator && self.navigator.userAgent;
    if (ua) headers['User-Agent'] = ua;
    const finalReferer = referer || pageUrl;
    if (finalReferer) headers['Referer'] = finalReferer;
    return headers;
}

function ensureNativePort() {
    if (nativePort) return nativePort;

    // Implement backoff: don't retry too quickly after a disconnect
    const timeSinceDisconnect = Date.now() - lastDisconnectTime;
    if (timeSinceDisconnect < RECONNECT_BACKOFF_MS) {
        return null; // Still in cooldown, don't retry yet
    }

    try {
        nativePort = chrome.runtime.connectNative(HOST_NAME);
        let badgeSet = false;
        nativePort.onMessage.addListener((message) => {
            if (!message || !message.id) return;
            if (message.output_path && !badgeSet) {
                badgeSet = true;
                chrome.action.setBadgeText({ text: 'KH' });
            }
        });
        nativePort.onDisconnect.addListener(() => {
            nativePort = null;
            lastDisconnectTime = Date.now();
        });
    } catch (e) {
        console.error('Failed to connect to native host:', e);
        nativePort = null;
        lastDisconnectTime = Date.now();
        return null;
    }

    return nativePort;
}

function sendToNative(payload) {
    const port = ensureNativePort();
    if (!port) {
        console.warn('Native bridge not available, queueing download for retry:', payload.url);
        return;
    }

    try {
        port.postMessage(payload);
    } catch (e) {
        console.error('Failed to send message to native host:', e);
        nativePort = null;
        lastDisconnectTime = Date.now();
    }
}

function dedupeKey(details) {
    return `${details.tabId}:${details.url}`;
}

// Returns true if the request is a duplicate within the TTL window.
// Also evicts entries that are older than the TTL and trims the map to
// RECENT_REQUESTS_MAX entries (oldest-first) to bound memory usage.
function isDuplicateRequest(key) {
    const now = Date.now();
    const last = recentRequests.get(key);
    if (last !== undefined && now - last < RECENT_REQUESTS_TTL_MS) {
        return true;
    }

    // Evict expired entries, then trim to size cap.
    for (const [k, ts] of recentRequests) {
        if (now - ts >= RECENT_REQUESTS_TTL_MS) recentRequests.delete(k);
    }
    if (recentRequests.size >= RECENT_REQUESTS_MAX) {
        // Delete the oldest entry (Maps iterate insertion order).
        recentRequests.delete(recentRequests.keys().next().value);
    }

    recentRequests.set(key, now);
    return false;
}

function scoreStreamCandidate(url) {
    if (!url) return 0;
    if (/\.m3u8(\?|$)/i.test(url)) return 4;
    if (/\.mpd(\?|$)/i.test(url)) return 4;
    if (/videoplayback/i.test(url)) return 3;
    if (/^blob:/i.test(url)) return 1;
    return 0;
}

function rememberBestStream(tabId, payload) {
    if (typeof tabId !== 'number' || tabId < 0) return;
    const current = latestStreamByTab.get(tabId);
    const nextScore = scoreStreamCandidate(payload.url);
    const currentScore = current ? scoreStreamCandidate(current.url) : 0;
    if (!current || nextScore >= currentScore) {
        latestStreamByTab.set(tabId, payload);
    }
}

function hasUsableStreamCandidate(payload) {
    return Boolean(payload && payload.url && !payload.url.startsWith('blob:'));
}

function waitForUsableStreamCandidate(tabId, timeoutMs = 3000) {
    return new Promise((resolve) => {
        const startedAt = Date.now();

        function check() {
            const candidate = latestStreamByTab.get(tabId);
            if (hasUsableStreamCandidate(candidate)) {
                resolve(candidate);
                return;
            }

            if (Date.now() - startedAt >= timeoutMs) {
                resolve(candidate || null);
                return;
            }

            setTimeout(check, 250);
        }

        check();
    });
}

function canHandleDownload(url) {
    // Khukri cannot handle these URL schemes
    if (!url) return false;
    if (url.startsWith('blob:')) return false;
    if (url.startsWith('data:')) return false;
    return true;
}

function isBridgeConnected() {
    return nativePort !== null;
}

chrome.downloads.onCreated.addListener((downloadItem) => {
    // Check if we can handle this URL
    const url = downloadItem.finalUrl || downloadItem.url;
    if (!canHandleDownload(url)) {
        // Let browser handle it
        return;
    }

    // Only cancel the download if the bridge is connected
    if (!isBridgeConnected()) {
        // Bridge not connected, let browser's default download handler work
        return;
    }

    chrome.downloads.cancel(downloadItem.id);

    sendToNative({
        type: 'queue_download',
        url: url,
        filename: normalizeFilename(downloadItem.filename, url),
        size: downloadItem.fileSize || null,
        source: 'browser',
        pageUrl: downloadItem.referrer || null,
        customHeaders: buildCustomHeaders({ referer: downloadItem.referrer, pageUrl: downloadItem.referrer })
    });
});

chrome.webRequest.onBeforeRequest.addListener(
    (details) => {
        if (!isTargetStream(details.url)) return;

        if (isDuplicateRequest(dedupeKey(details))) return;

        const payload = {
            type: 'queue_download',
            url: details.url,
            filename: normalizeFilename('', details.url),
            size: null,
            source: 'stream',
            pageUrl: details.documentUrl || details.initiator || null,
            customHeaders: buildCustomHeaders({
                referer: details.initiator || null,
                pageUrl: details.documentUrl || null
            })
        };

        rememberBestStream(details.tabId, payload);
    },
    { urls: ['<all_urls>'], types: ['xmlhttprequest', 'media'] }
);

chrome.runtime.onMessage.addListener((message, sender) => {
    if (!message || !message.type) return;

    if (message.type === 'stream_detected') {
        const payload = {
            type: 'queue_download',
            url: message.url || message.pageUrl || sender.tab?.url || '',
            filename: normalizeFilename(message.filename, message.url || message.pageUrl || sender.tab?.url || ''),
            size: null,
            source: 'stream',
            pageUrl: message.pageUrl || sender.tab?.url || null,
            customHeaders: buildCustomHeaders({
                referer: message.pageUrl || sender.tab?.url || null,
                pageUrl: message.pageUrl || sender.tab?.url || null
            })
        };

        rememberBestStream(sender.tab?.id, payload);
        return;
    }

    if (message.type === 'queue_download' && message.source === 'blade') {
        const senderTabId = sender.tab?.id;
        const senderTabUrl = sender.tab?.url;
        const initial = latestStreamByTab.get(senderTabId) || null;

        (async () => {
            const remembered = hasUsableStreamCandidate(initial)
                ? initial
                : await waitForUsableStreamCandidate(senderTabId);

            const resolvedUrl = hasUsableStreamCandidate(remembered)
                ? remembered.url
                : (message.url && !message.url.startsWith('blob:') ? message.url : '') ||
                senderTabUrl ||
                message.pageUrl ||
                '';

            const resolvedPageUrl =
                remembered?.pageUrl || message.pageUrl || senderTabUrl || null;

            sendToNative({
                type: 'queue_download',
                url: resolvedUrl,
                filename: normalizeFilename(message.filename, resolvedUrl || senderTabUrl || 'video'),
                size: null,
                quality: 'best',
                source: 'blade',
                pageUrl: resolvedPageUrl,
                customHeaders: remembered?.customHeaders || buildCustomHeaders({
                    referer: resolvedPageUrl,
                    pageUrl: resolvedPageUrl
                })
            });
        })();
    }
});

function resetDismissedSites() {
    chrome.storage.local.remove('dismissed_sites');
}

// Content scripts are registered dynamically so that <all_urls> can be an
// optional (user-grantable) host permission rather than a mandatory one.
// Chrome fires the webRequest listener only for origins where permission has
// been granted, so no extra guarding is needed there.
const CONTENT_SCRIPT_REGISTRATIONS = [
    {
        id: 'khukri-main-world',
        matches: ['<all_urls>'],
        js: ['content-script-main.js'],
        runAt: 'document_start',
        world: 'MAIN',
    },
    {
        id: 'khukri-isolated',
        matches: ['<all_urls>'],
        js: ['content-script.js'],
        runAt: 'document_idle',
        allFrames: true,
        world: 'ISOLATED',
    },
    {
        id: 'khukri-blade',
        matches: ['<all_urls>'],
        js: ['blade-ui.js'],
        runAt: 'document_idle',
        allFrames: true,
        world: 'ISOLATED',
    },
];

async function registerContentScripts() {
    try {
        // Unregister stale entries before re-registering so updates apply cleanly.
        const existing = await chrome.scripting.getRegisteredContentScripts();
        const existingIds = existing.map((s) => s.id);
        const toUnregister = CONTENT_SCRIPT_REGISTRATIONS
            .map((s) => s.id)
            .filter((id) => existingIds.includes(id));

        if (toUnregister.length > 0) {
            await chrome.scripting.unregisterContentScripts({ ids: toUnregister });
        }

        await chrome.scripting.registerContentScripts(CONTENT_SCRIPT_REGISTRATIONS);
    } catch (e) {
        console.error('Khukri: failed to register content scripts:', e);
    }
}

async function unregisterContentScripts() {
    try {
        const ids = CONTENT_SCRIPT_REGISTRATIONS.map((s) => s.id);
        await chrome.scripting.unregisterContentScripts({ ids });
    } catch {}
}

// Re-register scripts whenever <all_urls> is granted (e.g. after first-run
// permission prompt or after a user re-grants a previously revoked permission).
chrome.permissions.onAdded.addListener(async (permissions) => {
    if (permissions.origins && permissions.origins.includes('<all_urls>')) {
        await registerContentScripts();
    }
});

// Remove scripts when the broad permission is revoked so we don't leave
// stale registrations that would fail silently.
chrome.permissions.onRemoved.addListener(async (permissions) => {
    if (permissions.origins && permissions.origins.includes('<all_urls>')) {
        await unregisterContentScripts();
    }
});

// On action click: request <all_urls> if not yet granted (requires a user
// gesture — this is the only valid place to call permissions.request in MV3).
chrome.action.onClicked.addListener(async () => {
    const already = await chrome.permissions.contains({ origins: ['<all_urls>'] });
    if (!already) {
        const granted = await chrome.permissions.request({ origins: ['<all_urls>'] });
        if (granted) {
            await registerContentScripts();
        }
        return;
    }
    // If permission already granted, ensure scripts are registered (idempotent).
    await registerContentScripts();
});

chrome.runtime.onInstalled.addListener(async () => {
    resetDismissedSites();
    // If the user already granted <all_urls> (e.g. extension update path),
    // register content scripts immediately.
    const has = await chrome.permissions.contains({ origins: ['<all_urls>'] });
    if (has) {
        await registerContentScripts();
    }
});

chrome.runtime.onStartup.addListener(() => {
    resetDismissedSites();
});
