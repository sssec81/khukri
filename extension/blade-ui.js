(function () {
    const PILL_ID = 'khukri-blade-pill';
    let showTimer = null;

    const ICON_DOWNLOAD = `
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 4v12m0 0l-4.5-4.5M12 16l4.5-4.5M5 19h14"
                stroke="#FF9F1C" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>`;

    const ICON_CLOSE = `
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M18 6L6 18M6 6l12 12"
                stroke="currentColor" stroke-width="2.3" stroke-linecap="round"/>
        </svg>`;

    const PILL_STYLE = `
        @keyframes khukri-in {
            from { opacity: 0; transform: translateY(12px) scale(0.94); }
            to   { opacity: 1; transform: translateY(0) scale(1); }
        }
        @keyframes khukri-out {
            from { opacity: 1; transform: translateY(0) scale(1); }
            to   { opacity: 0; transform: translateY(10px) scale(0.94); }
        }
        @keyframes khukri-shimmer {
            0%   { background-position: -300% 0; }
            100% { background-position: 300% 0; }
        }

        #${PILL_ID} {
            position: absolute;
            top: 16px;
            right: 16px;
            z-index: 2147483647;
            display: inline-flex;
            align-items: stretch;
            cursor: pointer;
            border-radius: 16px;
            overflow: hidden;
            background:
                linear-gradient(145deg, rgba(45, 90, 39, 0.96), rgba(17, 18, 22, 0.96));
            border: 1px solid rgba(255, 159, 28, 0.34);
            box-shadow:
                0 14px 34px rgba(0, 0, 0, 0.4),
                0 0 0 1px rgba(45, 90, 39, 0.28);
            font-family: -apple-system, 'SF Pro Display', 'Segoe UI Variable Display',
                         BlinkMacSystemFont, 'Helvetica Neue', sans-serif;
            user-select: none;
            outline: none;
            backdrop-filter: blur(18px) saturate(1.25);
            -webkit-backdrop-filter: blur(18px) saturate(1.25);
            animation: khukri-in 0.32s cubic-bezier(0.34, 1.4, 0.64, 1) both;
            transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
        }

        #${PILL_ID}::after {
            content: '';
            position: absolute;
            inset: 0;
            background: linear-gradient(
                100deg,
                transparent 20%,
                rgba(255, 159, 28, 0.08) 50%,
                transparent 80%
            );
            background-size: 300% 100%;
            animation: khukri-shimmer 4s ease infinite;
            pointer-events: none;
        }

        #${PILL_ID}:hover {
            transform: translateY(-1px);
            border-color: rgba(255, 159, 28, 0.6);
            box-shadow:
                0 18px 38px rgba(0, 0, 0, 0.45),
                0 0 0 1px rgba(255, 159, 28, 0.18);
        }

        #${PILL_ID} .kh-icon-zone {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 52px;
            background: rgba(255, 159, 28, 0.12);
            border-right: 1px solid rgba(255, 159, 28, 0.12);
        }

        #${PILL_ID} .kh-icon-circle {
            width: 30px;
            height: 30px;
            border-radius: 50%;
            background: rgba(255, 159, 28, 0.12);
            border: 1px solid rgba(255, 159, 28, 0.26);
            display: flex;
            align-items: center;
            justify-content: center;
        }

        #${PILL_ID} .kh-content {
            display: flex;
            flex-direction: column;
            justify-content: center;
            padding: 12px 15px 12px 13px;
            gap: 4px;
        }

        #${PILL_ID} .kh-title {
            font-size: 14px;
            font-weight: 700;
            line-height: 1;
            color: #fff;
            white-space: nowrap;
        }

        #${PILL_ID} .kh-brand {
            color: #FF9F1C;
        }

        #${PILL_ID} .kh-sub {
            font-size: 10px;
            font-weight: 700;
            letter-spacing: 0.08em;
            color: rgba(255, 255, 255, 0.68);
            white-space: nowrap;
        }

        #${PILL_ID} .kh-sep {
            width: 1px;
            background: rgba(255, 255, 255, 0.08);
            margin: 10px 0;
        }

        #${PILL_ID} .kh-close {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 40px;
            background: none;
            border: none;
            cursor: pointer;
            color: rgba(255, 255, 255, 0.3);
            transition: color 0.15s ease, background 0.15s ease;
        }

        #${PILL_ID} .kh-close:hover {
            color: rgba(255, 255, 255, 0.82);
            background: rgba(255, 255, 255, 0.06);
        }

        #${PILL_ID}.kh-dismissing {
            animation: khukri-out 0.22s cubic-bezier(0.4, 0, 1, 1) both !important;
            pointer-events: none;
        }
    `;

    function ensureStyle() {
        if (document.getElementById(`${PILL_ID}-style`)) return;
        const style = document.createElement('style');
        style.id = `${PILL_ID}-style`;
        style.textContent = PILL_STYLE;
        document.head.appendChild(style);
    }

    function hasExtensionContext() {
        try {
            return Boolean(chrome?.runtime?.id && chrome?.storage?.local);
        } catch {
            return false;
        }
    }

    function safeStorageGet(keys, callback) {
        if (!hasExtensionContext()) return false;
        try {
            chrome.storage.local.get(keys, (result) => {
                if (chrome.runtime?.lastError) return;
                if (!hasExtensionContext()) return;
                callback(result);
            });
            return true;
        } catch {
            return false;
        }
    }

    function safeStorageSet(value) {
        if (!hasExtensionContext()) return false;
        try {
            chrome.storage.local.set(value, () => void chrome.runtime?.lastError);
            return true;
        } catch {
            return false;
        }
    }

    function safeSendMessage(message) {
        if (!hasExtensionContext()) return false;
        try {
            chrome.runtime.sendMessage(message, () => void chrome.runtime?.lastError);
            return true;
        } catch {
            return false;
        }
    }

    function dismiss(pill, origin) {
        pill.classList.add('kh-dismissing');
        pill.addEventListener('animationend', () => pill.remove(), { once: true });
        safeStorageGet(['dismissed_sites'], (result) => {
            const next = Array.isArray(result.dismissed_sites) ? result.dismissed_sites.slice() : [];
            if (!next.includes(origin)) next.push(origin);
            safeStorageSet({ dismissed_sites: next });
        });
    }

    function hidePill(pill) {
        pill.classList.add('kh-dismissing');
        pill.addEventListener('animationend', () => pill.remove(), { once: true });
    }

    function clearPill() {
        clearTimeout(showTimer);
        showTimer = null;
        const existing = document.getElementById(PILL_ID);
        if (existing) existing.remove();
    }

    function queueDownload() {
        return safeSendMessage({
            type: 'queue_download',
            source: 'blade',
            filename: document.title || 'video',
            pageUrl: location.href
        });
    }

    function injectPill() {
        const origin = location.origin;

        if (!safeStorageGet(['dismissed_sites'], (result) => {
            if (result.dismissed_sites && result.dismissed_sites.includes(origin)) return;
            if (document.getElementById(PILL_ID)) return;

            ensureStyle();

            const pill = document.createElement('div');
            pill.id = PILL_ID;
            pill.setAttribute('role', 'button');
            pill.setAttribute('tabindex', '0');
            pill.setAttribute('aria-label', 'Download this video with Khukri');
            pill.innerHTML = `
                <div class="kh-icon-zone">
                    <div class="kh-icon-circle">${ICON_DOWNLOAD}</div>
                </div>
                <div class="kh-content">
                    <div class="kh-title">Download <span class="kh-brand">Khukri</span></div>
                    <div class="kh-sub">HD · MP4 READY</div>
                </div>
                <div class="kh-sep"></div>
                <button class="kh-close" title="Dismiss" aria-label="Dismiss">${ICON_CLOSE}</button>
            `;

            pill.querySelector('.kh-close').addEventListener('click', (event) => {
                event.stopPropagation();
                dismiss(pill, origin);
            });

            pill.addEventListener('click', (event) => {
                if (event.target.closest('.kh-close')) return;
                if (!queueDownload()) {
                    pill.remove();
                    return;
                }
                hidePill(pill);
            });

            pill.addEventListener('keydown', (event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    pill.click();
                }
                if (event.key === 'Escape') dismiss(pill, origin);
            });

            const container =
                document.querySelector('.html5-video-player') ||
                document.querySelector('#movie_player') ||
                document.querySelector('video')?.parentElement ||
                document.body;

            if (container !== document.body && getComputedStyle(container).position === 'static') {
                container.style.position = 'relative';
            }

            container.appendChild(pill);
        })) {
            clearPill();
        }
    }

    function schedulePill() {
        if (showTimer) return;
        if (document.getElementById(PILL_ID)) return;
        showTimer = window.setTimeout(() => {
            showTimer = null;
            injectPill();
        }, 1500);
    }

    function watchVideoPresence() {
        const hasVideo = Boolean(document.querySelector('video'));
        if (hasVideo) {
            schedulePill();
        } else {
            clearPill();
        }
    }

    if (window.location.hostname.includes('youtube.com')) {
        window.addEventListener('yt-navigate-finish', () => {
            clearPill();
            watchVideoPresence();
        });
    }

    new MutationObserver(() => {
        if (!document.getElementById(PILL_ID) && document.querySelector('video')) {
            schedulePill();
        }
    }).observe(document.documentElement, { childList: true, subtree: true });

    window.addEventListener('beforeunload', () => clearTimeout(showTimer));

    watchVideoPresence();
})();
