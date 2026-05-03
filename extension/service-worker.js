const HOST_NAME = 'com.khukri.host';
const STREAM_PATTERNS = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i];

let nativePort = null;
let lastDisconnectTime = 0;
const RECONNECT_BACKOFF_MS = 1000; // Wait 1 second before retrying after disconnect
const recentRequests = new Map();
const RECENT_REQUESTS_MAX = 500;
const RECENT_REQUESTS_TTL_MS = 4000;
const latestStreamByTab = new Map();
const QUALITY_STORAGE_KEY = 'quality_preferences';
const QUALITY_DEFAULT = 'best';
const INTERCEPT_MODE_KEY = 'intercept_mode';
const INTERCEPT_MODE_ASK = 'ask';
const INTERCEPT_MODE_AUTO = 'auto';

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
        return false;
    }

    try {
        port.postMessage(payload);
        return true;
    } catch (e) {
        console.error('Failed to send message to native host:', e);
        nativePort = null;
        lastDisconnectTime = Date.now();
        return false;
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

function loadQualityPreference(origin) {
    return new Promise((resolve) => {
        if (!origin) {
            resolve(QUALITY_DEFAULT);
            return;
        }

        chrome.storage.local.get([QUALITY_STORAGE_KEY], (result) => {
            if (chrome.runtime.lastError) {
                resolve(QUALITY_DEFAULT);
                return;
            }

            const prefs = result && typeof result[QUALITY_STORAGE_KEY] === 'object'
                ? result[QUALITY_STORAGE_KEY]
                : null;
            const saved = prefs && typeof prefs[origin] === 'string' ? prefs[origin] : '';
            resolve(saved || QUALITY_DEFAULT);
        });
    });
}

function originFromUrl(url) {
    try {
        return url ? new URL(url).origin : '';
    } catch {
        return '';
    }
}

function canHandleDownload(url) {
    // Khukri cannot handle these URL schemes
    if (!url) return false;
    if (url.startsWith('blob:')) return false;
    if (url.startsWith('data:')) return false;
    return true;
}

function loadInterceptMode() {
    return new Promise((resolve) => {
        chrome.storage.local.get([INTERCEPT_MODE_KEY], (result) => {
            if (chrome.runtime.lastError) {
                resolve(INTERCEPT_MODE_ASK);
                return;
            }
            const mode = result?.[INTERCEPT_MODE_KEY];
            resolve(mode === INTERCEPT_MODE_AUTO ? INTERCEPT_MODE_AUTO : INTERCEPT_MODE_ASK);
        });
    });
}

function startDownloadInKhukri(downloadItem) {
    const url = downloadItem.finalUrl || downloadItem.url;
    if (!ensureNativePort()) {
        return false;
    }

    const sent = sendToNative({
        type: 'queue_download',
        url,
        filename: normalizeFilename(downloadItem.filename, url),
        size: downloadItem.fileSize || null,
        source: 'browser',
        pageUrl: downloadItem.referrer || null,
        customHeaders: buildCustomHeaders({ referer: downloadItem.referrer, pageUrl: downloadItem.referrer })
    });

    if (sent) {
        chrome.downloads.cancel(downloadItem.id);
    }
    return sent;
}

function sendDownloadPrompt(downloadItem) {
    const tabId = downloadItem.tabId;
    if (typeof tabId !== 'number' || tabId < 0) {
        return false;
    }

    chrome.tabs.sendMessage(tabId, {
        type: 'khukri_prompt_download',
        payload: {
            id: downloadItem.id,
            url: downloadItem.finalUrl || downloadItem.url,
            filename: normalizeFilename(downloadItem.filename, downloadItem.finalUrl || downloadItem.url),
            size: downloadItem.fileSize || null,
            referrer: downloadItem.referrer || null
        }
    }, () => {
        if (chrome.runtime.lastError) {
            // If prompt UI is unavailable on the page, keep interception reliable.
            startDownloadInKhukri(downloadItem);
        }
    });
    return true;
}

chrome.downloads.onCreated.addListener((downloadItem) => {
    // Check if we can handle this URL
    const url = downloadItem.finalUrl || downloadItem.url;
    if (!canHandleDownload(url)) {
        // Let browser handle it
        return;
    }

    void loadInterceptMode().then((mode) => {
        if (mode === INTERCEPT_MODE_ASK) {
            const promptSent = sendDownloadPrompt(downloadItem);
            if (!promptSent) {
                startDownloadInKhukri(downloadItem);
            }
            return;
        }
        startDownloadInKhukri(downloadItem);
    });
});

function onStreamRequest(details) {
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
}

async function syncWebRequestListener() {
    const hasAllUrls = await chrome.permissions.contains({ origins: ['<all_urls>'] });
    const isRegistered = chrome.webRequest.onBeforeRequest.hasListener(onStreamRequest);

    if (hasAllUrls && !isRegistered) {
        chrome.webRequest.onBeforeRequest.addListener(
            onStreamRequest,
            { urls: ['<all_urls>'], types: ['xmlhttprequest', 'media'] }
        );
        return;
    }

    if (!hasAllUrls && isRegistered) {
        chrome.webRequest.onBeforeRequest.removeListener(onStreamRequest);
    }
}

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
            const origin = originFromUrl(senderTabUrl);
            const remembered = hasUsableStreamCandidate(initial)
                ? initial
                : await waitForUsableStreamCandidate(senderTabId);
            const requestedQuality = message.quality || await loadQualityPreference(origin);

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
                quality: requestedQuality,
                source: 'blade',
                pageUrl: resolvedPageUrl,
                customHeaders: remembered?.customHeaders || buildCustomHeaders({
                    referer: resolvedPageUrl,
                    pageUrl: resolvedPageUrl
                })
            });
        })();
    }

    if (message.type === 'khukri_prompt_decision') {
        const payload = message.payload || {};
        const action = payload.action;
        const downloadId = payload.id;
        const url = payload.url || '';
        const filename = normalizeFilename(payload.filename || '', url);
        const referrer = payload.referrer || null;

        if (payload.remember === true && (action === 'start' || action === 'keep')) {
            chrome.storage.local.set({
                [INTERCEPT_MODE_KEY]: action === 'start' ? INTERCEPT_MODE_AUTO : INTERCEPT_MODE_ASK
            }, () => void chrome.runtime.lastError);
        }

        if (action === 'start') {
            if (typeof downloadId === 'number') {
                chrome.downloads.cancel(downloadId, () => void chrome.runtime.lastError);
            }

            sendToNative({
                type: 'queue_download',
                url,
                filename,
                size: payload.size || null,
                source: 'browser',
                pageUrl: referrer,
                customHeaders: buildCustomHeaders({ referer: referrer, pageUrl: referrer })
            });
        }
    }
});

function resetDismissedSites() {
    chrome.storage.local.remove('dismissed_sites');
}

chrome.permissions.onAdded.addListener(async () => {
    await syncWebRequestListener();
});

chrome.permissions.onRemoved.addListener(async () => {
    await syncWebRequestListener();
});

chrome.runtime.onInstalled.addListener(async () => {
    resetDismissedSites();
    await syncWebRequestListener();
});

chrome.runtime.onStartup.addListener(async () => {
    resetDismissedSites();
    await syncWebRequestListener();
});

(async () => {
    try {
        await syncWebRequestListener();
    } catch (error) {
        console.error('Khukri: boot-time listener sync failed:', error);
    }
})();
