(function () {
    const PILL_ID = 'khukri-blade-pill';
    const PROMPT_ID = 'khukri-download-prompt';
    const DISMISSED_SITES_KEY = 'dismissed_sites';
    const DISMISSED_SITE_TTL_MS = 7 * 24 * 60 * 60 * 1000;
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
            top: 18px;
            right: 18px;
            z-index: 2147483647;
            display: flex;
            align-items: center;
            cursor: pointer;
            min-width: 304px;
            max-width: 356px;
            border-radius: 18px;
            overflow: hidden;
            background:
                radial-gradient(circle at top left, rgba(255, 166, 43, 0.16), transparent 34%),
                linear-gradient(155deg, rgba(34, 61, 30, 0.97), rgba(15, 18, 22, 0.98) 58%);
            border: 1px solid rgba(255, 166, 43, 0.42);
            box-shadow:
                0 22px 52px rgba(0, 0, 0, 0.42),
                0 0 0 1px rgba(255, 184, 77, 0.08) inset;
            font-family: "Segoe UI Variable Display", "Aptos", -apple-system, 'SF Pro Display',
                         BlinkMacSystemFont, 'Helvetica Neue', sans-serif;
            user-select: none;
            outline: none;
            backdrop-filter: blur(20px) saturate(1.24);
            -webkit-backdrop-filter: blur(20px) saturate(1.24);
            animation: khukri-in 0.32s cubic-bezier(0.34, 1.4, 0.64, 1) both;
            transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
        }

        #${PILL_ID}::after {
            content: '';
            position: absolute;
            inset: 0;
            background: linear-gradient(
                110deg,
                transparent 20%,
                rgba(255, 184, 77, 0.08) 50%,
                transparent 80%
            );
            background-size: 300% 100%;
            animation: khukri-shimmer 5.4s ease infinite;
            pointer-events: none;
        }

        #${PILL_ID}:hover {
            transform: translateY(-2px);
            border-color: rgba(255, 184, 77, 0.68);
            box-shadow:
                0 26px 56px rgba(0, 0, 0, 0.46),
                0 0 0 1px rgba(255, 184, 77, 0.14) inset;
        }

        #${PILL_ID} .kh-main {
            display: grid;
            grid-template-columns: 48px minmax(0, 1fr) max-content 34px;
            align-items: center;
            min-height: 56px;
            width: 100%;
        }

        #${PILL_ID} .kh-icon-zone {
            display: flex;
            align-items: center;
            justify-content: center;
            align-self: stretch;
            background:
                linear-gradient(180deg, rgba(255, 166, 43, 0.18), rgba(255, 166, 43, 0.08));
            border-right: 1px solid rgba(255, 184, 77, 0.14);
        }

        #${PILL_ID} .kh-icon-circle {
            width: 30px;
            height: 30px;
            border-radius: 50%;
            background:
                linear-gradient(180deg, rgba(255, 176, 74, 0.18), rgba(255, 159, 28, 0.08));
            border: 1px solid rgba(255, 184, 77, 0.34);
            display: flex;
            align-items: center;
            justify-content: center;
            box-shadow: 0 8px 18px rgba(0, 0, 0, 0.2);
        }

        #${PILL_ID} .kh-content {
            display: flex;
            flex-direction: column;
            justify-content: center;
            padding: 8px 6px 8px 12px;
            gap: 2px;
            min-width: 0;
        }

        #${PILL_ID} .kh-kicker {
            font-size: 9px;
            font-weight: 700;
            letter-spacing: 0.18em;
            text-transform: uppercase;
            color: rgba(255, 245, 224, 0.5);
            white-space: nowrap;
        }

        #${PILL_ID} .kh-title {
            font-size: 13px;
            font-weight: 700;
            line-height: 1.1;
            color: #fff;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            letter-spacing: -0.02em;
        }

        #${PILL_ID} .kh-brand {
            color: #ffad32;
        }

        #${PILL_ID} .kh-sub {
            font-size: 9px;
            font-weight: 700;
            letter-spacing: 0.14em;
            color: rgba(241, 247, 238, 0.64);
            text-transform: uppercase;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }

        #${PILL_ID} .kh-close {
            display: flex;
            align-items: center;
            justify-content: center;
            align-self: stretch;
            background: none;
            border: none;
            cursor: pointer;
            color: rgba(255, 255, 255, 0.34);
            transition: color 0.15s ease, background 0.15s ease;
        }

        #${PILL_ID} .kh-close:hover {
            color: rgba(255, 255, 255, 0.82);
            background: rgba(255, 255, 255, 0.05);
        }

        #${PILL_ID} .kh-quality-wrap {
            display: flex;
            align-items: center;
            gap: 0;
            justify-content: flex-end;
            padding: 0 6px 0 0;
            min-width: 0;
            width: 100%;
            position: relative;
        }

        #${PILL_ID} .kh-quality-label {
            font-size: 9px;
            font-weight: 700;
            letter-spacing: 0.14em;
            color: rgba(255, 245, 224, 0.52);
            text-transform: uppercase;
            white-space: nowrap;
            display: none;
        }

        #${PILL_ID} .kh-quality-select {
            width: 88px;
            min-width: 88px;
            max-width: 104px;
            border: 1px solid rgba(255, 184, 77, 0.22);
            border-radius: 999px;
            background: rgba(10, 12, 16, 0.72);
            color: #fff;
            font-size: 11px;
            font-weight: 700;
            height: 34px;
            padding: 0 10px;
            outline: none;
            cursor: pointer;
            box-shadow: 0 1px 0 rgba(255, 255, 255, 0.04) inset;
            transition: width 0.16s ease, min-width 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease;
        }

        #${PILL_ID} .kh-quality-select:focus {
            border-color: rgba(255, 184, 77, 0.7);
            box-shadow: 0 0 0 3px rgba(255, 184, 77, 0.14);
        }

        #${PILL_ID}:hover .kh-quality-select,
        #${PILL_ID}:focus-within .kh-quality-select {
            width: 104px;
            min-width: 104px;
        }

        #${PILL_ID}.kh-dismissing {
            animation: khukri-out 0.22s cubic-bezier(0.4, 0, 1, 1) both !important;
            pointer-events: none;
        }

        @media (max-width: 960px) {
            #${PILL_ID} {
                top: 12px;
                right: 12px;
                min-width: 280px;
                max-width: min(332px, calc(100vw - 24px));
            }

            #${PILL_ID} .kh-main {
                grid-template-columns: 44px minmax(0, 1fr) max-content 32px;
                min-height: 52px;
            }

            #${PILL_ID} .kh-quality-select {
                width: 82px;
                min-width: 82px;
                height: 32px;
                font-size: 11px;
            }

            #${PILL_ID}:hover .kh-quality-select,
            #${PILL_ID}:focus-within .kh-quality-select {
                width: 92px;
                min-width: 92px;
            }
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

    function readDismissedSites(result) {
        const raw = result?.[DISMISSED_SITES_KEY];
        if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {};

        const now = Date.now();
        const active = {};
        for (const [origin, expiresAt] of Object.entries(raw)) {
            if (typeof expiresAt === 'number' && expiresAt > now) {
                active[origin] = expiresAt;
            }
        }
        return active;
    }

    function writeDismissedSites(sites) {
        if (!sites || Object.keys(sites).length === 0) {
            return safeStorageSet({ [DISMISSED_SITES_KEY]: {} });
        }
        return safeStorageSet({ [DISMISSED_SITES_KEY]: sites });
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

    function ensurePromptStyle() {
        if (document.getElementById(`${PROMPT_ID}-style`)) return;
        const style = document.createElement('style');
        style.id = `${PROMPT_ID}-style`;
        style.textContent = `
        #${PROMPT_ID}{position:fixed;right:20px;bottom:20px;z-index:2147483647;width:min(420px,calc(100vw - 24px));border:1px solid rgba(255,159,28,.34);background:linear-gradient(145deg,rgba(45,90,39,.96),rgba(17,18,22,.96));border-radius:12px;box-shadow:0 16px 34px rgba(0,0,0,.42);color:#fff;font-family:-apple-system,'SF Pro Display','Segoe UI Variable Display',BlinkMacSystemFont,'Helvetica Neue',sans-serif;padding:12px}
        #${PROMPT_ID} .khp-title{font-weight:700;font-size:13px;margin-bottom:4px}
        #${PROMPT_ID} .khp-sub{font-size:11px;color:rgba(255,255,255,.72);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-bottom:10px}
        #${PROMPT_ID} .khp-actions{display:flex;gap:8px}
        #${PROMPT_ID} button{border:1px solid rgba(255,255,255,.16);border-radius:8px;padding:8px 10px;cursor:pointer;font-size:12px;font-weight:700;color:#fff;background:rgba(255,255,255,.06)}
        #${PROMPT_ID} .khp-primary{background:rgba(74,140,68,.95);border-color:rgba(74,140,68,1)}
        #${PROMPT_ID} .khp-foot{margin-top:9px;display:flex;align-items:center;gap:6px;font-size:11px;color:rgba(255,255,255,.78)}
        `;
        document.head.appendChild(style);
    }

    function removePrompt() {
        document.getElementById(PROMPT_ID)?.remove();
    }

    function showDownloadPrompt(payload) {
        ensurePromptStyle();
        removePrompt();
        const root = document.createElement('div');
        root.id = PROMPT_ID;
        root.innerHTML = `
          <div class="khp-title">Download intercepted</div>
          <div class="khp-sub" title="${payload.filename || payload.url || ''}">${payload.filename || payload.url || ''}</div>
          <div class="khp-actions">
            <button class="khp-primary" type="button" data-action="start">Start in Khukri</button>
            <button type="button" data-action="keep">Keep in Browser</button>
          </div>
          <label class="khp-foot"><input type="checkbox" id="khukri-prompt-remember" />Remember this choice</label>
        `;
        document.documentElement.appendChild(root);

        root.addEventListener('click', (event) => {
            const button = event.target.closest('button[data-action]');
            if (!button) return;
            const remember = Boolean(root.querySelector('#khukri-prompt-remember')?.checked);
            safeSendMessage({
                type: 'khukri_prompt_decision',
                payload: {
                    action: button.dataset.action,
                    remember,
                    id: payload.id,
                    url: payload.url,
                    filename: payload.filename,
                    size: payload.size,
                    referrer: payload.referrer
                }
            });
            removePrompt();
        });
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
        safeStorageGet([DISMISSED_SITES_KEY], (result) => {
            const next = readDismissedSites(result);
            next[origin] = Date.now() + DISMISSED_SITE_TTL_MS;
            writeDismissedSites(next);
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

        if (!safeStorageGet([DISMISSED_SITES_KEY], (result) => {
            const dismissedSites = readDismissedSites(result);
            if (dismissedSites[origin]) {
                if (Object.keys(dismissedSites).length !== Object.keys(result?.[DISMISSED_SITES_KEY] || {}).length) {
                    writeDismissedSites(dismissedSites);
                }
                return;
            }
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
            const kicker = document.createElement('div');
            kicker.className = 'kh-kicker';
            kicker.textContent = 'Quick Save';
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
            content.appendChild(kicker);
            content.appendChild(title);
            content.appendChild(sub);

            const closeBtn = document.createElement('button');
            closeBtn.className = 'kh-close';
            closeBtn.title = 'Dismiss';
            closeBtn.setAttribute('aria-label', 'Dismiss');
            closeBtn.innerHTML = ICON_CLOSE;

            main.appendChild(iconZone);
            main.appendChild(content);
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

            main.appendChild(qualityWrap);
            main.appendChild(closeBtn);
            pill.appendChild(main);

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

    if (hasExtensionContext()) {
        chrome.runtime.onMessage.addListener((message) => {
            if (message?.type === 'khukri_prompt_download' && message.payload) {
                showDownloadPrompt(message.payload);
            }
        });
    }
})();
