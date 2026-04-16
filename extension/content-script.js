(function () {
    const patterns = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i, /^blob:/i];
    const detected = new Set();

    function emit(url, context) {
        if (!url || detected.has(url)) return;
        detected.add(url);
        chrome.runtime.sendMessage({
            type: 'stream_detected',
            url,
            filename: document.title || 'video',
            pageUrl: location.href,
            context
        });
    }

    function maybeEmit(url, context) {
        if (url && patterns.some((pattern) => pattern.test(url))) {
            emit(String(url), context);
        }
    }

    const originalFetch = window.fetch;
    window.fetch = async function (...args) {
        const response = await originalFetch.apply(this, args);
        try {
            const requestUrl = response.url || args[0]?.url || args[0];
            maybeEmit(requestUrl, { method: 'fetch' });
        } catch {}
        return response;
    };

    const originalOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (method, url, ...rest) {
        maybeEmit(url, { method: 'xhr' });
        return originalOpen.call(this, method, url, ...rest);
    };

    function watchVideoSrc() {
        const videos = document.querySelectorAll('video');
        for (const video of videos) {
            if (video.currentSrc) {
                maybeEmit(video.currentSrc, { method: 'video-current-src' });
            }
            video.addEventListener('loadedmetadata', () => {
                maybeEmit(video.currentSrc || video.src, { method: 'loadedmetadata' });
            }, { once: true });
        }
    }

    new MutationObserver(() => watchVideoSrc()).observe(document.documentElement, {
        childList: true,
        subtree: true
    });

    watchVideoSrc();
})();
