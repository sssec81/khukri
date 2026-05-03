// Runs in the ISOLATED world (Chrome MV3 default).
// Responsibilities:
//   1. Relay stream URLs detected by the MAIN world script (content-script-main.js)
//      from window.postMessage to chrome.runtime.sendMessage.
//   2. Observe <video> element src changes via DOM APIs (accessible from isolated world).
//
// Fetch/XHR patching must NOT be done here — isolated-world scripts cannot
// intercept the page's own network calls.
(function () {
    const MSG_TYPE = 'khukri_v1_stream';
    const detected = new Set();

    // Guard against "Extension context invalidated" errors that occur when the
    // extension is reloaded while this content script is still alive in an old
    // tab. Once the context is dead, chrome.runtime.id becomes undefined and
    // any chrome.runtime call throws. We check once and then stop trying.
    function isExtensionAlive() {
        try {
            return Boolean(chrome.runtime?.id);
        } catch {
            return false;
        }
    }

    function sendDetected(url, filename, pageUrl, context) {
        if (!url || detected.has(url)) return;
        if (!isExtensionAlive()) return; // stale context — silently drop
        detected.add(url);
        try {
            chrome.runtime.sendMessage({
                type: 'stream_detected',
                url,
                filename,
                pageUrl,
                context,
            });
        } catch (e) {
            // Context died between the check and the call — ignore silently.
            // Nothing we can do from a stale content script.
        }
    }

    // Relay messages posted by content-script-main.js running in the MAIN world.
    // Guard with event.source === window to ignore cross-frame messages.
    window.addEventListener('message', (event) => {
        if (event.source !== window) return;
        if (!event.data || event.data.type !== MSG_TYPE) return;
        const { url, filename, pageUrl, context } = event.data;
        sendDetected(url, filename || document.title || 'video', pageUrl, context);
    });

    // DOM-level <video> observation — works from isolated world.
    const videoPatterns = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i];

    function inspectVideo(video) {
        const src = video.currentSrc || video.src;
        if (src && videoPatterns.some((p) => p.test(src))) {
            sendDetected(src, document.title || 'video', location.href, {
                method: 'video-element',
            });
        }
    }

    function watchVideoSrc() {
        document.querySelectorAll('video').forEach((video) => {
            if (video.currentSrc) inspectVideo(video);
            video.addEventListener('loadedmetadata', () => inspectVideo(video), { once: true });
        });
    }

    new MutationObserver(watchVideoSrc).observe(document.documentElement, {
        childList: true,
        subtree: true,
    });

    watchVideoSrc();
})();