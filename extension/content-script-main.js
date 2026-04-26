// Runs in the page's MAIN world so window.fetch / XHR patching actually
// intercepts the page's own network requests. Has no access to chrome.*
// APIs — detected URLs are forwarded to the isolated-world relay script
// via window.postMessage.
(function () {
    const MSG_TYPE = 'khukri_v1_stream';
    const patterns = [/\.m3u8(\?|$)/i, /\.mpd(\?|$)/i, /videoplayback/i, /^blob:/i];
    const detected = new Set();

    function emit(url, context) {
        if (!url || detected.has(url)) return;
        detected.add(url);
        window.postMessage(
            {
                type: MSG_TYPE,
                url: String(url),
                filename: document.title || 'video',
                pageUrl: location.href,
                context,
            },
            '*'
        );
    }

    function maybeEmit(url, context) {
        if (url && patterns.some((p) => p.test(url))) emit(String(url), context);
    }

    const originalFetch = window.fetch;
    window.fetch = async function (...args) {
        const response = await originalFetch.apply(this, args);
        try {
            maybeEmit(response.url || args[0]?.url || args[0], { method: 'fetch' });
        } catch {}
        return response;
    };

    const originalOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (method, url, ...rest) {
        maybeEmit(url, { method: 'xhr' });
        return originalOpen.call(this, method, url, ...rest);
    };
})();
