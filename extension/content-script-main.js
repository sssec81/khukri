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
    // Use a Proxy so sites that check fetch.toString() or fetch.name still
    // see the native function — avoids fingerprinting detection.
    window.fetch = new Proxy(originalFetch, {
        apply(target, thisArg, args) {
            const result = Reflect.apply(target, thisArg, args);
            result.then((response) => {
                try {
                    maybeEmit(response.url || args[0]?.url || args[0], { method: 'fetch' });
                } catch {}
            }).catch(() => {});
            return result;
        },
    });

    const originalOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = new Proxy(originalOpen, {
        apply(target, thisArg, args) {
            try {
                maybeEmit(args[1], { method: 'xhr' });
            } catch {}
            return Reflect.apply(target, thisArg, args);
        },
    });
})();
