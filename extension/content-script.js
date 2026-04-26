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

    function sendDetected(url, filename, pageUrl, context) {
        if (!url || detected.has(url)) return;
        detected.add(url);
        chrome.runtime.sendMessage({
            type: 'stream_detected',
            url,
            filename,
            pageUrl,
            context,
        });
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
