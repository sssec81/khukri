const HOST_NAME = 'com.khukri.host';
const STREAM_PATTERNS = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i];

let nativePort = null;
const recentRequests = new Map();
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

    nativePort = chrome.runtime.connectNative(HOST_NAME);
    nativePort.onMessage.addListener((message) => {
        if (!message || !message.id) return;
        if (message.output_path) {
            chrome.action.setBadgeText({ text: 'KH' });
        }
    });
    nativePort.onDisconnect.addListener(() => {
        nativePort = null;
    });

    return nativePort;
}

function sendToNative(payload) {
    ensureNativePort().postMessage(payload);
}

function dedupeKey(details) {
    return `${details.tabId}:${details.url}`;
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

chrome.downloads.onCreated.addListener((downloadItem) => {
    chrome.downloads.cancel(downloadItem.id);

    sendToNative({
        type: 'queue_download',
        url: downloadItem.finalUrl || downloadItem.url,
        filename: normalizeFilename(downloadItem.filename, downloadItem.url),
        size: downloadItem.fileSize || null,
        source: 'browser',
        pageUrl: downloadItem.referrer || null,
        customHeaders: buildCustomHeaders({ referer: downloadItem.referrer, pageUrl: downloadItem.referrer })
    });
});

chrome.webRequest.onBeforeRequest.addListener(
    (details) => {
        if (!isTargetStream(details.url)) return;

        const key = dedupeKey(details);
        const now = Date.now();
        if (recentRequests.has(key) && now - recentRequests.get(key) < 4000) return;
        recentRequests.set(key, now);

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

chrome.runtime.onInstalled.addListener(() => {
    resetDismissedSites();
});

chrome.runtime.onStartup.addListener(() => {
    resetDismissedSites();
});
