(function () {
    const PILL_ID = 'khukri-blade-pill';
    const QUALITY_STORAGE_KEY = 'quality_preferences';
    const QUALITY_DEFAULT = 'best';
    const QUALITY_OPTIONS = [
        { value: 'best', label: 'Best', subtitle: 'BEST AVAILABLE' },
        { value: '1080p', label: '1080p', subtitle: '1080P CAP' },
        { value: '720p', label: '720p', subtitle: '720P CAP' },
        { value: 'audio-only', label: 'Audio Only', subtitle: 'MP3 EXTRACT' },
    ];
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
            display: flex;
            flex-direction: column;
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

        #${PILL_ID}:hover .kh-quality-wrap,
        #${PILL_ID}:focus-within .kh-quality-wrap {
            opacity: 1;
            transform: translateY(0);
            pointer-events: auto;
        }

        #${PILL_ID} .kh-main {
            display: inline-flex;
            align-items: stretch;
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

        #${PILL_ID} .kh-quality-wrap {
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 10px 12px 12px;
            border-top: 1px solid rgba(255, 255, 255, 0.08);
            background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(0, 0, 0, 0.12));
            opacity: 0;
            transform: translateY(-6px);
            pointer-events: none;
            transition: opacity 0.16s ease, transform 0.16s ease;
        }

        #${PILL_ID} .kh-quality-label {
            font-size: 10px;
            font-weight: 700;
            letter-spacing: 0.08em;
            color: rgba(255, 255, 255, 0.6);
            text-transform: uppercase;
            white-space: nowrap;
        }

        #${PILL_ID} .kh-quality-select {
            flex: 1 1 auto;
            min-width: 0;
            border: 1px solid rgba(255, 159, 28, 0.24);
            border-radius: 10px;
            background: rgba(9, 10, 14, 0.72);
            color: #fff;
            font-size: 12px;
            font-weight: 700;
            padding: 8px 10px;
            outline: none;
            cursor: pointer;
        }

        #${PILL_ID} .kh-quality-select:focus {
            border-color: rgba(255, 159, 28, 0.65);
            box-shadow: 0 0 0 2px rgba(255, 159, 28, 0.15);
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

    function qualityForOrigin(result, origin) {
        const prefs = result && typeof result[QUALITY_STORAGE_KEY] === 'object'
            ? result[QUALITY_STORAGE_KEY]
            : null;
        const saved = prefs && typeof prefs[origin] === 'string' ? prefs[origin] : '';
        return QUALITY_OPTIONS.some((option) => option.value === saved) ? saved : QUALITY_DEFAULT;
    }

    function saveQuality(origin, quality) {
        safeStorageGet([QUALITY_STORAGE_KEY], (result) => {
            const prefs = result && typeof result[QUALITY_STORAGE_KEY] === 'object'
                ? { ...result[QUALITY_STORAGE_KEY] }
                : {};
            prefs[origin] = quality;
            safeStorageSet({ [QUALITY_STORAGE_KEY]: prefs });
        });
    }

    function subtitleForQuality(quality) {
        const match = QUALITY_OPTIONS.find((option) => option.value === quality);
        return match ? match.subtitle : 'BEST AVAILABLE';
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

    function queueDownload(quality) {
        return safeSendMessage({
            type: 'queue_download',
            source: 'blade',
            filename: document.title || 'video',
            pageUrl: location.href,
            quality
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
            const selectedQuality = qualityForOrigin(result, origin);
            let activeQuality = selectedQuality;

            const main = document.createElement('div');
            main.className = 'kh-main';
            const iconZone = document.createElement('div');
            iconZone.className = 'kh-icon-zone';
            const iconCircle = document.createElement('div');
            iconCircle.className = 'kh-icon-circle';
            iconCircle.innerHTML = ICON_DOWNLOAD;
            iconZone.appendChild(iconCircle);

            const content = document.createElement('div');
            content.className = 'kh-content';
            const title = document.createElement('div');
            title.className = 'kh-title';
            title.appendChild(document.createTextNode('Download '));
            const brand = document.createElement('span');
            brand.className = 'kh-brand';
            brand.textContent = 'Khukri';
            title.appendChild(brand);
            const sub = document.createElement('div');
            sub.className = 'kh-sub';
            sub.textContent = subtitleForQuality(activeQuality);
            content.appendChild(title);
            content.appendChild(sub);

            const sep = document.createElement('div');
            sep.className = 'kh-sep';

            const closeBtn = document.createElement('button');
            closeBtn.className = 'kh-close';
            closeBtn.title = 'Dismiss';
            closeBtn.setAttribute('aria-label', 'Dismiss');
            closeBtn.innerHTML = ICON_CLOSE;

            main.appendChild(iconZone);
            main.appendChild(content);
            main.appendChild(sep);
            main.appendChild(closeBtn);

            const qualityWrap = document.createElement('div');
            qualityWrap.className = 'kh-quality-wrap';
            const qualityLabel = document.createElement('span');
            qualityLabel.className = 'kh-quality-label';
            qualityLabel.textContent = 'Quality';
            const qualitySelect = document.createElement('select');
            qualitySelect.className = 'kh-quality-select';
            qualitySelect.setAttribute('aria-label', 'Preferred video quality');
            for (const option of QUALITY_OPTIONS) {
                const node = document.createElement('option');
                node.value = option.value;
                node.textContent = option.label;
                qualitySelect.appendChild(node);
            }
            qualitySelect.value = activeQuality;
            qualityWrap.appendChild(qualityLabel);
            qualityWrap.appendChild(qualitySelect);

            pill.appendChild(main);
            pill.appendChild(qualityWrap);

            closeBtn.addEventListener('click', (event) => {
                event.stopPropagation();
                dismiss(pill, origin);
            });

            qualityWrap.addEventListener('click', (event) => {
                event.stopPropagation();
            });

            qualitySelect.addEventListener('change', (event) => {
                activeQuality = event.target.value || QUALITY_DEFAULT;
                sub.textContent = subtitleForQuality(activeQuality);
                saveQuality(origin, activeQuality);
            });

            pill.addEventListener('click', (event) => {
                if (event.target.closest('.kh-close')) return;
                if (event.target.closest('.kh-quality-wrap')) return;
                if (!queueDownload(activeQuality)) {
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
