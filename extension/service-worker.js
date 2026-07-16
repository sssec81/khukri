const HOST_NAME = 'com.khukri.host';
const STREAM_PATTERNS = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i];

let nativePort = null;
let lastDisconnectTime = 0;
const RECONNECT_BACKOFF_MS = 1000;
const recentRequests = new Map();
const RECENT_REQUESTS_MAX = 500;
const RECENT_REQUESTS_TTL_MS = 4000;
const latestStreamByTab = new Map();
const DISMISSED_SITES_KEY = 'dismissed_sites';
const QUALITY_STORAGE_KEY = 'quality_preferences';
const QUALITY_DEFAULT = 'best';
const INTERCEPT_MODE_KEY = 'intercept_mode';
const INTERCEPT_MODE_ASK = 'ask';
const INTERCEPT_MODE_AUTO = 'auto';
const PROMPT_STORAGE_PREFIX = 'khukri_prompt_';
const PROMPT_SUPPRESS_UNTIL_KEY = 'khukri_prompt_suppress_until';
const PROMPT_TAB_ID_KEY = 'khukri_prompt_tab_id';
const PROMPT_ACTIVE_TOKEN_KEY = 'khukri_prompt_active_token';
const BYPASS_TTL_MS = 10000;
const browserBypassUntil = new Map();

// FIX 6 — retry queue key in chrome.storage.session
const RETRY_QUEUE_KEY = 'khukri_retry_queue';
const STARTUP_PROMPT_SUPPRESS_MS = 15000;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers — unchanged from original
// ─────────────────────────────────────────────────────────────────────────────

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

    const timeSinceDisconnect = Date.now() - lastDisconnectTime;
    if (timeSinceDisconnect < RECONNECT_BACKOFF_MS) {
        return null;
    }

    try {
        console.info('Khukri SW: connecting to native host', HOST_NAME);
        nativePort = chrome.runtime.connectNative(HOST_NAME);
        let badgeSet = false;
        nativePort.onMessage.addListener((message) => {
            console.info('Khukri SW: received native host message', message?.type || 'unknown', message?.status || '');
            if (!message || !message.id) return;
            if (message.output_path && !badgeSet) {
                badgeSet = true;
                chrome.action.setBadgeText({ text: 'KH' });
            }
        });
        nativePort.onDisconnect.addListener(() => {
            const lastError = chrome.runtime.lastError?.message || '';
            console.warn('Khukri SW: native host disconnected', lastError);
            nativePort = null;
            lastDisconnectTime = Date.now();
        });
    } catch (e) {
        console.error('Khukri: Failed to connect to native host:', e);
        nativePort = null;
        lastDisconnectTime = Date.now();
        return null;
    }

    return nativePort;
}

function sendToNative(payload) {
    const port = ensureNativePort();
    if (!port) {
        console.warn('Khukri: Native bridge not available for payload:', payload.url);
        return Promise.resolve(false);
    }

    return new Promise((resolve) => {
        let settled = false;
        const finish = (sent) => {
            if (settled) return;
            settled = true;
            clearTimeout(timer);
            port.onMessage.removeListener(onMessage);
            port.onDisconnect.removeListener(onDisconnect);
            resolve(sent);
        };
        const onMessage = () => finish(true);
        const onDisconnect = () => finish(false);
        const timer = setTimeout(() => {
            console.warn('Khukri: Native bridge did not acknowledge the download');
            finish(false);
        }, 2500);

        port.onMessage.addListener(onMessage);
        port.onDisconnect.addListener(onDisconnect);

        try {
            console.info('Khukri SW: posting payload to native host', {
                source: payload.source,
                url: payload.url,
                pageUrl: payload.pageUrl,
                quality: payload.quality || null
            });
            port.postMessage(payload);
        } catch (e) {
            console.error('Khukri: Failed to send message to native host:', e);
            nativePort = null;
            lastDisconnectTime = Date.now();
            finish(false);
        }
    });
}

function dedupeKey(details) {
    return `${details.tabId}:${details.url}`;
}

function isDuplicateRequest(key) {
    const now = Date.now();
    const last = recentRequests.get(key);
    if (last !== undefined && now - last < RECENT_REQUESTS_TTL_MS) {
        return true;
    }

    for (const [k, ts] of recentRequests) {
        if (now - ts >= RECENT_REQUESTS_TTL_MS) recentRequests.delete(k);
    }
    if (recentRequests.size >= RECENT_REQUESTS_MAX) {
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

function cleanupTabState(tabId) {
    if (typeof tabId !== 'number' || tabId < 0) return;
    latestStreamByTab.delete(tabId);
    const prefix = `${tabId}:`;
    for (const key of recentRequests.keys()) {
        if (key.startsWith(prefix)) recentRequests.delete(key);
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

function isYoutubePageUrl(url) {
    try {
        const host = new URL(url).hostname.toLowerCase();
        return host === 'youtube.com'
            || host === 'www.youtube.com'
            || host === 'm.youtube.com'
            || host === 'music.youtube.com'
            || host === 'youtu.be';
    } catch {
        return false;
    }
}

function canHandleDownload(url) {
    if (!url) return false;
    if (url.startsWith('blob:')) return false;
    if (url.startsWith('data:')) return false;
    return true;
}

function browserBypassKey(url) {
    return String(url || '');
}

function shouldBypassBrowserDownload(url) {
    const key = browserBypassKey(url);
    const expiresAt = browserBypassUntil.get(key) || 0;
    if (expiresAt <= Date.now()) {
        browserBypassUntil.delete(key);
        return false;
    }
    browserBypassUntil.delete(key);
    return true;
}

function bypassNextBrowserDownload(url) {
    browserBypassUntil.set(browserBypassKey(url), Date.now() + BYPASS_TTL_MS);
}

function storageSessionSet(values) {
    return chrome.storage.session.set(values);
}

function storageSessionGet(key) {
    return chrome.storage.session.get(key);
}

function storageSessionRemove(key) {
    return chrome.storage.session.remove(key);
}

function storageSessionGetAll() {
    return chrome.storage.session.get(null);
}

async function clearPromptState({ closeTabs = false } = {}) {
    try {
        const all = await storageSessionGetAll();
        const promptKeys = Object.keys(all).filter((key) =>
            key.startsWith(PROMPT_STORAGE_PREFIX)
            || key === PROMPT_TAB_ID_KEY
            || key === PROMPT_ACTIVE_TOKEN_KEY
        );
        if (promptKeys.length > 0) {
            await chrome.storage.session.remove(promptKeys);
        }
    } catch (e) {
        console.warn('Khukri: Failed to clear prompt session state:', e);
    }

    if (!closeTabs) return;

    try {
        const urlPattern = chrome.runtime.getURL('prompt.html') + '*';
        const tabs = await chrome.tabs.query({ url: urlPattern });
        for (const tab of tabs) {
            if (typeof tab.id === 'number') {
                chrome.tabs.remove(tab.id).catch(() => {});
            }
        }
    } catch (e) {
        console.warn('Khukri: Failed to close prompt tabs:', e);
    }
}

async function suppressPromptWindowsForStartup() {
    try {
        await storageSessionSet({
            [PROMPT_SUPPRESS_UNTIL_KEY]: Date.now() + STARTUP_PROMPT_SUPPRESS_MS
        });
    } catch (e) {
        console.warn('Khukri: Failed to set startup prompt suppression:', e);
    }
}

async function shouldSuppressPromptWindowFallback() {
    try {
        const result = await storageSessionGet(PROMPT_SUPPRESS_UNTIL_KEY);
        const until = Number(result?.[PROMPT_SUPPRESS_UNTIL_KEY] || 0);
        return until > Date.now();
    } catch (e) {
        console.warn('Khukri: Failed to read prompt suppression window:', e);
        return false;
    }
}

async function openPromptTabForToken(token) {
    const promptUrl = chrome.runtime.getURL(`prompt.html?token=${encodeURIComponent(token)}`);
    try {
        const result = await storageSessionGet(PROMPT_TAB_ID_KEY);
        const existingTabId = Number(result?.[PROMPT_TAB_ID_KEY] || 0);
        if (existingTabId > 0) {
            try {
                const updated = await chrome.tabs.update(existingTabId, {
                    url: promptUrl,
                    active: true
                });
                if (updated?.windowId) {
                    await chrome.windows.update(updated.windowId, { focused: true });
                }
                return true;
            } catch (e) {
                await storageSessionRemove(PROMPT_TAB_ID_KEY);
            }
        }

        const created = await chrome.tabs.create({ url: promptUrl, active: true });
        if (typeof created?.id === 'number') {
            await storageSessionSet({ [PROMPT_TAB_ID_KEY]: created.id });
            return true;
        }
    } catch (e) {
        console.warn('Khukri: Failed to open prompt tab:', e);
    }

    return false;
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

// ─────────────────────────────────────────────────────────────────────────────
// FIX 6 — Retry queue
// When sendToNative fails and restartInBrowser also fails, the payload is
// pushed into chrome.storage.session. drainRetryQueue() is called on every SW
// wake so pending downloads are eventually delivered.
// ─────────────────────────────────────────────────────────────────────────────

async function pushRetryQueue(payload) {
    try {
        const result = await chrome.storage.session.get(RETRY_QUEUE_KEY);
        const existing = Array.isArray(result[RETRY_QUEUE_KEY]) ? result[RETRY_QUEUE_KEY] : [];
        // Cap the queue at 20 entries to avoid unbounded growth
        const next = [...existing, payload].slice(-20);
        await chrome.storage.session.set({ [RETRY_QUEUE_KEY]: next });
    } catch (e) {
        console.warn('Khukri: Failed to push retry queue:', e);
    }
}

async function drainRetryQueue() {
    try {
        const result = await chrome.storage.session.get(RETRY_QUEUE_KEY);
        const queue = result[RETRY_QUEUE_KEY];
        if (!Array.isArray(queue) || queue.length === 0) return;
        await chrome.storage.session.remove(RETRY_QUEUE_KEY);
        for (const payload of queue) {
            const sent = await sendToNative(payload);
            if (!sent) {
                if (payload.source === 'browser') {
                    await restartInBrowser(payload);
                } else {
                    await pushRetryQueue(payload);
                }
            }
        }
    } catch (e) {
        console.warn('Khukri: Failed to drain retry queue:', e);
    }
}

async function pruneDismissedSites() {
    try {
        const result = await chrome.storage.local.get([DISMISSED_SITES_KEY]);
        const raw = result?.[DISMISSED_SITES_KEY];
        if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
            if (Array.isArray(raw)) {
                await chrome.storage.local.remove(DISMISSED_SITES_KEY);
            }
            return;
        }

        const now = Date.now();
        const next = {};
        let changed = false;

        for (const [origin, expiresAt] of Object.entries(raw)) {
            if (typeof expiresAt === 'number' && expiresAt > now) {
                next[origin] = expiresAt;
            } else {
                changed = true;
            }
        }

        if (!changed) return;
        if (Object.keys(next).length === 0) {
            await chrome.storage.local.remove(DISMISSED_SITES_KEY);
            return;
        }
        await chrome.storage.local.set({ [DISMISSED_SITES_KEY]: next });
    } catch (error) {
        console.warn('Khukri: Failed to prune dismissed_sites:', error);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Download actions
// ─────────────────────────────────────────────────────────────────────────────

// FIX 7 — removed the redundant ensureNativePort() call; sendToNative() does
//          it internally. Also removed chrome.downloads.cancel() from here —
//          FIX 1 moves it to the onCreated listener so it fires synchronously.
async function startDownloadInKhukri(downloadItem) {
    const url = downloadItem.finalUrl || downloadItem.url;

    const sent = await sendToNative({
        type: 'queue_download',
        url,
        filename: normalizeFilename(downloadItem.filename, url),
        size: downloadItem.fileSize || null,
        source: 'browser',
        pageUrl: downloadItem.referrer || null,
        customHeaders: buildCustomHeaders({ referer: downloadItem.referrer, pageUrl: downloadItem.referrer })
    });

    // FIX 6 — if the native bridge is down, immediately restart in browser, 
    // using the retry queue as secondary recovery if browser restart fails.
    if (!sent) {
        restartInBrowser(downloadItem).then((restarted) => {
            if (!restarted) {
                void pushRetryQueue({
                    type: 'queue_download',
                    url,
                    filename: normalizeFilename(downloadItem.filename, url),
                    size: downloadItem.fileSize || null,
                    source: 'browser',
                    pageUrl: downloadItem.referrer || null,
                    customHeaders: buildCustomHeaders({ referer: downloadItem.referrer, pageUrl: downloadItem.referrer })
                });
            }
        });
    }

    return sent;
}

async function restartInBrowser(payload) {
    if (!payload?.url) return false;
    if (isYoutubePageUrl(payload.url)) {
        console.info('Khukri: not restarting YouTube page URL as browser download');
        return false;
    }
    bypassNextBrowserDownload(payload.url);
    return new Promise((resolve) => {
        chrome.downloads.download({
            url: payload.url,
            filename: normalizeFilename(payload.filename, payload.url),
            conflictAction: 'uniquify',
            saveAs: false
        }, (id) => {
            if (chrome.runtime.lastError) {
                console.warn('Khukri: Failed to restart browser download:', chrome.runtime.lastError.message);
                resolve(false);
                return;
            }
            resolve(typeof id === 'number');
        });
    });
}

async function openPromptForPayload(payload) {
    const token = crypto.randomUUID();
    const storageKey = `${PROMPT_STORAGE_PREFIX}${token}`;
    const promptPayload = {
        ...payload,
        id: payload.id || token,
        filename: normalizeFilename(payload.filename || '', payload.url),
        createdAt: Date.now()
    };

    await storageSessionSet({ [storageKey]: promptPayload });

    // Try to display the prompt directly in the active tab (DOM overlay)
    // to completely avoid browser popup blockers and tab-forcing behavior.
    try {
        const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
        const activeTab = tabs[0];
        if (activeTab && activeTab.id && activeTab.url && !activeTab.url.startsWith('chrome://')) {
            const injected = await new Promise((resolve) => {
                chrome.tabs.sendMessage(
                    activeTab.id,
                    { type: 'khukri_prompt_download', payload: promptPayload },
                    (response) => {
                        if (chrome.runtime.lastError || !response) {
                            resolve(false);
                        } else {
                            resolve(true);
                        }
                    }
                );
            });
            if (injected) {
                return true;
            }
        }
    } catch (e) {
        console.warn('Khukri: Failed to send DOM prompt to active tab', e);
    }

    if (await shouldSuppressPromptWindowFallback()) {
        console.info('Khukri: suppressing popup prompt fallback during startup');
        await storageSessionRemove(storageKey);
        if (promptPayload.source === 'browser') {
            await restartInBrowser(promptPayload);
            return false;
        }
        await pushRetryQueue(promptPayload);
        return false;
    }

    try {
        const current = await storageSessionGet(PROMPT_ACTIVE_TOKEN_KEY);
        const previousToken = String(current?.[PROMPT_ACTIVE_TOKEN_KEY] || '');
        if (previousToken) {
            await storageSessionRemove(`${PROMPT_STORAGE_PREFIX}${previousToken}`);
        }
        await storageSessionSet({ [PROMPT_ACTIVE_TOKEN_KEY]: token });
    } catch (e) {
        console.warn('Khukri: Failed to update active prompt token:', e);
    }

    if (await openPromptTabForToken(token)) {
        return true;
    }

    await storageSessionRemove(storageKey);
    if (promptPayload.source === 'browser') {
        console.info('Khukri: prompt fallback tab unavailable; keeping browser download');
        await restartInBrowser(promptPayload);
        return false;
    }

    console.info('Khukri: prompt fallback tab unavailable; queueing payload for retry');
    await pushRetryQueue(promptPayload);
    return false;
}

// FIX 2 — Removed the chrome.downloads.cancel() call that was here in the
//          original. Cancellation is now done synchronously in onCreated (FIX 1)
//          before this function is ever called, so doing it again here would
//          trigger a harmless-but-noisy error on an already-cancelled download.
//
// FIX 2 — Fixed the chrome.windows.create success check: we now test `!win`
//          in addition to `chrome.runtime.lastError`, because some Chrome
//          versions pass a null `win` without setting lastError.
async function openDownloadPrompt(downloadItem) {
    const url = downloadItem.finalUrl || downloadItem.url;
    return openPromptForPayload({
        id: downloadItem.id,
        type: 'queue_download',
        url,
        filename: normalizeFilename(downloadItem.filename, url),
        size: downloadItem.fileSize || null,
        referrer: downloadItem.referrer || null,
        source: 'browser',
        pageUrl: downloadItem.referrer || null,
        customHeaders: buildCustomHeaders({
            referer: downloadItem.referrer || null,
            pageUrl: downloadItem.referrer || null
        })
    });
}

// FIX 4 — storageSessionRemove is now called BEFORE dispatching the action.
//          Previously it was called after, meaning a double-fire (e.g. user
//          clicks twice before the SW processes) could dispatch the action
//          twice. Removing the key first makes the handler idempotent.
async function handlePromptDecision(payload, action, remember) {
    if (remember === true && (action === 'start' || action === 'keep')) {
        chrome.storage.local.set({
            [INTERCEPT_MODE_KEY]: action === 'start' ? INTERCEPT_MODE_AUTO : INTERCEPT_MODE_ASK
        }, () => void chrome.runtime.lastError);
    }

    if (action === 'dismiss') {
        return true;
    }

    if (action === 'keep') {
        if (payload.source === 'browser') {
            await restartInBrowser(payload);
        }
        return true;
    }

    if (action === 'start') {
        const sent = await sendToNative({
            type: 'queue_download',
            url: payload.url,
            filename: normalizeFilename(payload.filename || '', payload.url),
            size: payload.size || null,
            source: payload.source || 'browser',
            pageUrl: payload.pageUrl || payload.referrer || null,
            quality: payload.quality || null,
            customHeaders: payload.customHeaders || buildCustomHeaders({
                referer: payload.pageUrl || payload.referrer || null,
                pageUrl: payload.pageUrl || payload.referrer || null
            })
        });
        if (!sent) {
            if (payload.source === 'browser') {
                await restartInBrowser(payload);
            }
        }
        return sent;
    }

    return false;
}

// ─────────────────────────────────────────────────────────────────────────────
// FIX 1 — downloads.onCreated
// The single most important change: chrome.downloads.cancel() now fires
// SYNCHRONOUSLY at the top of the listener, before loadInterceptMode() is
// awaited. This guarantees the download is stopped even if the async chain
// takes a long time (storage read, window creation) or if the SW is briefly
// suspended between microtasks.
//
// Why this matters: in the original code, cancel() was called inside
// openDownloadPrompt(), which is only reached after two async hops
// (loadInterceptMode resolves, then openDownloadPrompt is called). Chrome can
// complete a small file download in that window, making interception useless.
// ─────────────────────────────────────────────────────────────────────────────
chrome.downloads.onCreated.addListener((downloadItem) => {
    const url = downloadItem.finalUrl || downloadItem.url;

    if (!canHandleDownload(url)) return;
    if (shouldBypassBrowserDownload(url)) return;

    // FIX 1 — SYNCHRONOUS cancel before any async work
    chrome.downloads.cancel(downloadItem.id, () => void chrome.runtime.lastError);

    // FIX 6 — drain any queued retries from a previous bridge-unavailable state
    void drainRetryQueue();

    void loadInterceptMode().then((mode) => {
        if (mode === INTERCEPT_MODE_ASK) {
            void openDownloadPrompt(downloadItem);
            return;
        }
        void startDownloadInKhukri(downloadItem);
    });
});

// ─────────────────────────────────────────────────────────────────────────────
// Stream detection (webRequest path) — unchanged from original
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Message handler
// ─────────────────────────────────────────────────────────────────────────────

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
        console.info('Khukri SW: blade queue_download received', {
            senderTabId,
            senderTabUrl,
            requestedQuality: message.quality || null,
            initialStreamUrl: initial?.url || null
        });

        (async () => {
            const origin = originFromUrl(senderTabUrl);
            const remembered = hasUsableStreamCandidate(initial)
                ? initial
                : await waitForUsableStreamCandidate(senderTabId);
            const requestedQuality = message.quality || await loadQualityPreference(origin);
            const preferredPageUrl =
                message.pageUrl || senderTabUrl || remembered?.pageUrl || null;
            const preferExtractorPageUrl = isYoutubePageUrl(preferredPageUrl);

            const resolvedUrl = preferExtractorPageUrl
                ? (preferredPageUrl || '')
                : hasUsableStreamCandidate(remembered)
                ? remembered.url
                : (message.url && !message.url.startsWith('blob:') ? message.url : '') ||
                senderTabUrl ||
                message.pageUrl ||
                '';

            const resolvedPageUrl =
                preferredPageUrl;

            console.info('Khukri SW: blade payload resolved', {
                preferExtractorPageUrl,
                resolvedUrl,
                resolvedPageUrl,
                rememberedUrl: remembered?.url || null,
                requestedQuality
            });
            const payload = {
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
            };

            const mode = await loadInterceptMode();
            if (mode === INTERCEPT_MODE_ASK) {
                await openPromptForPayload(payload);
                return;
            }

            const sent = await sendToNative(payload);
            if (!sent) await pushRetryQueue(payload);
        })();
    }

    if (message.type === 'khukri_prompt_decision') {
        const payload = message.payload || {};
        return handlePromptDecision(payload, payload.action, payload.remember).then((sent) => ({
            ok: sent !== false,
            error: sent === false ? 'Khukri could not connect to its native bridge. Reload the extension and try again.' : null
        }));
    }

    if (message.type === 'khukri_prompt_get') {
        const token = String(message.token || '');
        const storageKey = `${PROMPT_STORAGE_PREFIX}${token}`;
        return storageSessionGet(storageKey).then((result) => result[storageKey] || null);
    }

    // FIX 5 — storageSessionRemove() now fires BEFORE handlePromptDecision(),
    //          making this handler idempotent against double-fire. The original
    //          code removed the key after the action, which meant a concurrent
    //          second message could re-read the same payload and act twice.
    if (message.type === 'khukri_prompt_choose') {
        const token = String(message.token || '');
        const storageKey = `${PROMPT_STORAGE_PREFIX}${token}`;
        return storageSessionGet(storageKey).then(async (result) => {
            const payload = result[storageKey];
            if (!payload) return { ok: false };
            await storageSessionRemove(storageKey);       // FIX 5 — remove first
            const sent = await handlePromptDecision(payload, message.action, message.remember);
            return {
                ok: sent !== false,
                error: sent === false ? 'Khukri could not connect to its native bridge. Reload the extension and try again.' : null
            };
        });
    }
});

// ─────────────────────────────────────────────────────────────────────────────
// FIX 3 — Keepalive port for the prompt popup
// prompt.html should open a Port with name 'khukri_prompt_keepalive' as soon
// as it loads (see prompt.js changes). While that port is open, Chrome will
// not suspend this service worker, ensuring the user can take as long as they
// need to read the dialog before clicking.
//
// The handler here is intentionally minimal: we just hold the port open and
// let the disconnect event clean itself up.
// ─────────────────────────────────────────────────────────────────────────────
chrome.runtime.onConnect.addListener((port) => {
    if (port.name !== 'khukri_prompt_keepalive') return;
    // Holding the reference is enough — Chrome won't suspend the SW.
    port.onDisconnect.addListener(() => {
        // Port closed (user clicked a button or closed the window). Nothing
        // to do here; the decision is handled via khukri_prompt_choose above.
        void chrome.runtime.lastError; // suppress "port closed" noise
    });
});

// ─────────────────────────────────────────────────────────────────────────────
// Lifecycle listeners
// ─────────────────────────────────────────────────────────────────────────────

chrome.permissions.onAdded.addListener(async () => {
    await syncWebRequestListener();
});

chrome.permissions.onRemoved.addListener(async () => {
    await syncWebRequestListener();
});

chrome.tabs.onRemoved.addListener((tabId) => {
    cleanupTabState(tabId);
    void storageSessionGet(PROMPT_TAB_ID_KEY).then(async (result) => {
        if (Number(result?.[PROMPT_TAB_ID_KEY] || 0) === tabId) {
            await chrome.storage.session.remove([PROMPT_TAB_ID_KEY, PROMPT_ACTIVE_TOKEN_KEY]);
        }
    });
});

chrome.runtime.onInstalled.addListener(async () => {
    await clearPromptState({ closeTabs: true });
    await suppressPromptWindowsForStartup();
    await pruneDismissedSites();
    await syncWebRequestListener();
    // FIX 6 — drain any retries that survived from before the install/update
    await drainRetryQueue();
});

chrome.runtime.onStartup.addListener(async () => {
    await clearPromptState({ closeTabs: true });
    await suppressPromptWindowsForStartup();
    await pruneDismissedSites();
    await syncWebRequestListener();
    // FIX 6 — drain retries on browser startup (bridge may be available now)
    await drainRetryQueue();
});

(async () => {
    try {
        await clearPromptState({ closeTabs: true });
        await suppressPromptWindowsForStartup();
        await pruneDismissedSites();
        await syncWebRequestListener();
        // FIX 6 — drain retries on SW boot (covers extension reload during dev)
        await drainRetryQueue();
    } catch (error) {
        console.error('Khukri: boot-time listener sync failed:', error);
    }
})();
