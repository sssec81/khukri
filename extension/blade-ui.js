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
    const QUALITY_HEIGHTS = {
        '1080p': 1080,
        '720p': 720,
    };
    let showTimer = null;

    const ICON_DOWNLOAD = `
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 4v12m0 0l-4.5-4.5M12 16l4.5-4.5M5 19h14"
                stroke="#10b981" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>`;

    const ICON_CLOSE = `
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M18 6L6 18M6 6l12 12"
                stroke="currentColor" stroke-width="2.3" stroke-linecap="round"/>
        </svg>`;

    const PILL_STYLE = `
        @keyframes khukri-in {
            from { opacity: 0; transform: translateY(-8px) scale(0.96); }
            to   { opacity: 1; transform: translateY(0) scale(1); }
        }
        @keyframes khukri-out {
            from { opacity: 1; transform: translateY(0) scale(1); }
            to   { opacity: 0; transform: translateY(-6px) scale(0.95); }
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
            align-items: stretch;
            cursor: pointer;
            width: max-content;
            max-width: calc(100vw - 32px);
            border-radius: 12px;
            background: rgba(12, 12, 14, 0.85);
            border: 1px solid rgba(255, 255, 255, 0.1);
            box-shadow:
                0 8px 32px rgba(0, 0, 0, 0.5),
                0 0 0 1px rgba(255,255,255,0.04) inset,
                0 0 24px rgba(16, 185, 129, 0.08);
            font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
            user-select: none;
            outline: none;
            overflow: hidden;
            backdrop-filter: blur(32px) saturate(1.4);
            -webkit-backdrop-filter: blur(32px) saturate(1.4);
            animation: khukri-in 0.35s cubic-bezier(0.16, 1, 0.3, 1) both;
            transition: box-shadow 0.2s ease, border-color 0.2s ease, transform 0.15s ease;
        }

        #${PILL_ID}:hover {
            border-color: rgba(255, 255, 255, 0.16);
            box-shadow:
                0 12px 40px rgba(0, 0, 0, 0.6),
                0 0 0 1px rgba(255,255,255,0.06) inset,
                0 0 32px rgba(16, 185, 129, 0.14);
        }

        #${PILL_ID} .kh-main {
            display: flex;
            align-items: center;
            gap: 0;
            position: relative;
            z-index: 1;
        }

        /* Left accent stripe */
        #${PILL_ID} .kh-icon-zone {
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 10px 12px;
            background: linear-gradient(180deg, rgba(16,185,129,0.18) 0%, rgba(16,185,129,0.08) 100%);
            border-right: 1px solid rgba(16,185,129,0.2);
        }

        #${PILL_ID} .kh-icon-circle {
            width: 28px;
            height: 28px;
            border-radius: 8px;
            background: rgba(16, 185, 129, 0.2);
            border: 1px solid rgba(16, 185, 129, 0.35);
            display: flex;
            align-items: center;
            justify-content: center;
            box-shadow: 0 0 10px rgba(16, 185, 129, 0.25);
        }

        #${PILL_ID} .kh-icon-circle svg {
            width: 13px;
            height: 13px;
        }

        #${PILL_ID} .kh-content {
            display: flex;
            flex-direction: column;
            justify-content: center;
            min-width: 0;
            padding: 9px 12px;
        }

        #${PILL_ID} .kh-kicker {
            display: block;
            font-size: 8.5px;
            font-weight: 700;
            letter-spacing: 0.12em;
            text-transform: uppercase;
            color: rgba(52, 211, 153, 0.75);
            margin-bottom: 2px;
        }

        #${PILL_ID} .kh-sub { display: none; }

        #${PILL_ID} .kh-title {
            font-size: 13px;
            font-weight: 600;
            line-height: 1.15;
            color: rgba(255,255,255,0.95);
            white-space: nowrap;
            letter-spacing: -0.01em;
            display: flex;
            align-items: center;
            gap: 4px;
        }

        #${PILL_ID} .kh-brand {
            color: #34d399;
            font-weight: 700;
        }

        #${PILL_ID} .kh-cap { display: none; }

        #${PILL_ID} .kh-quality-wrap {
            display: flex;
            align-items: center;
            padding: 0 4px 0 0;
        }

        #${PILL_ID} .kh-quality-label { display: none; }

        #${PILL_ID} .kh-quality-select {
            width: auto;
            border: 1px solid rgba(255,255,255,0.1);
            border-radius: 7px;
            background: rgba(255, 255, 255, 0.07);
            color: rgba(255,255,255,0.85);
            font-size: 11px;
            font-weight: 600;
            height: 26px;
            padding: 0 22px 0 9px;
            outline: none;
            cursor: pointer;
            transition: background 0.15s ease, border-color 0.15s ease;
            appearance: none;
            -webkit-appearance: none;
            background-image: url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='white' stroke-opacity='0.5' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
            background-repeat: no-repeat;
            background-position: right 7px center;
        }

        #${PILL_ID} .kh-quality-select:hover {
            background-color: rgba(255, 255, 255, 0.12);
            border-color: rgba(255,255,255,0.18);
        }

        #${PILL_ID} .kh-close {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 28px;
            height: 28px;
            margin: 8px 8px 8px 2px;
            border-radius: 8px;
            background: rgba(255, 255, 255, 0.04);
            border: none;
            cursor: pointer;
            color: rgba(255, 255, 255, 0.48);
            transition: color 0.15s ease, background 0.15s ease, transform 0.15s ease;
            flex-shrink: 0;
        }

        #${PILL_ID} .kh-close:hover {
            color: rgba(248, 113, 113, 0.95);
            background: rgba(248, 113, 113, 0.12);
            transform: translateY(-1px);
        }

        #${PILL_ID}.kh-dismissing {
            animation: khukri-out 0.2s cubic-bezier(0.4, 0, 1, 1) both !important;
            pointer-events: none;
        }

        @media (max-width: 960px) {
            #${PILL_ID} { top: 12px; right: 12px; }
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
        @keyframes khukri-prompt-in {
            from { opacity: 0; transform: translateY(12px) scale(0.97); }
            to   { opacity: 1; transform: translateY(0) scale(1); }
        }
        #${PROMPT_ID} {
            position: fixed;
            right: 18px;
            bottom: 18px;
            z-index: 2147483647;
            width: min(390px, calc(100vw - 24px));
            background:
                linear-gradient(180deg, rgba(22, 24, 25, 0.96), rgba(12, 13, 14, 0.96)),
                rgba(14, 14, 17, 0.94);
            border: 0.5px solid rgba(255, 255, 255, 0.18);
            border-radius: 16px;
            box-shadow:
                0 10px 24px rgba(0, 0, 0, 0.28),
                0 0 0 1px rgba(255,255,255,0.035) inset;
            color: #fff;
            font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
            overflow: hidden;
            backdrop-filter: blur(40px) saturate(1.5);
            -webkit-backdrop-filter: blur(40px) saturate(1.5);
            animation: khukri-prompt-in 0.32s cubic-bezier(0.16, 1, 0.3, 1) both;
        }
        #${PROMPT_ID} .khp-header {
            display: flex;
            align-items: center;
            gap: 11px;
            padding: 14px 14px 12px 15px;
            border-bottom: 1px solid rgba(255,255,255,0.06);
        }
        #${PROMPT_ID} .khp-icon {
            width: 30px;
            height: 30px;
            border-radius: 9px;
            background: rgba(52, 199, 89, 0.14);
            border: 1px solid rgba(52, 199, 89, 0.28);
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
        }
        #${PROMPT_ID} .khp-header-text {
            display: flex;
            flex-direction: column;
            gap: 3px;
            min-width: 0;
            flex: 1;
        }
        #${PROMPT_ID} .khp-close {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 28px;
            height: 28px;
            border-radius: 999px;
            background: rgba(255, 255, 255, 0.08);
            border: 0;
            cursor: pointer;
            color: rgba(255,255,255,0.58);
            transition: color 0.15s ease, background 0.15s ease, transform 0.15s ease;
            flex-shrink: 0;
            padding: 0;
        }
        #${PROMPT_ID} .khp-close:hover {
            color: rgba(248,113,113,0.95);
            background: rgba(248,113,113,0.12);
            transform: translateY(-1px);
        }
        #${PROMPT_ID} .khp-title-row {
            display: flex;
            align-items: center;
            gap: 7px;
            min-width: 0;
        }
        #${PROMPT_ID} .khp-title {
            font-size: 13px;
            font-weight: 600;
            letter-spacing: -0.01em;
            color: #fff;
            line-height: 1.2;
            white-space: nowrap;
        }
        #${PROMPT_ID} .khp-badge {
            display: inline-flex;
            align-items: center;
            height: 18px;
            padding: 0 7px;
            border-radius: 999px;
            background: rgba(52, 199, 89, 0.14);
            border: 1px solid rgba(52, 199, 89, 0.24);
            color: #6ee7a0;
            font-size: 10px;
            font-weight: 700;
            line-height: 1;
            flex-shrink: 0;
        }
        #${PROMPT_ID} .khp-sub {
            max-width: 300px;
            font-size: 11.5px;
            color: rgba(255, 255, 255, 0.58);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            line-height: 1.3;
        }
        #${PROMPT_ID} .khp-body {
            padding: 14px;
            display: flex;
            flex-direction: column;
            gap: 10px;
        }
        #${PROMPT_ID} .khp-actions {
            display: flex;
            flex-direction: column;
            gap: 8px;
        }
        #${PROMPT_ID} button {
            width: 100%;
            border: 0;
            border-radius: 10px;
            min-height: 40px;
            padding: 10px 13px;
            cursor: pointer;
            font-size: 12.5px;
            font-weight: 700;
            color: rgba(255, 255, 255, 0.62);
            background: rgba(255, 255, 255, 0.075);
            transition: background 0.15s ease, color 0.15s ease, transform 0.15s ease;
            line-height: 1;
            white-space: nowrap;
        }
        #${PROMPT_ID} button:hover {
            background: rgba(255, 255, 255, 0.11);
            color: rgba(255,255,255,0.82);
        }
        #${PROMPT_ID} button:active {
            transform: scale(0.97);
            opacity: 0.85;
        }
        #${PROMPT_ID} .khp-primary {
            display: inline-flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
            background: #34c759;
            color: #06351f;
            box-shadow: none;
        }
        #${PROMPT_ID} .khp-primary:hover {
            background: #3fdb66;
            color: #042715;
        }
        #${PROMPT_ID} .khp-button-icon {
            display: inline-flex;
            align-items: center;
            justify-content: center;
            width: 14px;
            height: 14px;
            flex-shrink: 0;
        }
        #${PROMPT_ID} .khp-foot {
            display: flex;
            align-items: center;
            gap: 8px;
            min-height: 28px;
            margin-top: 2px;
            padding-top: 10px;
            border-top: 1px solid rgba(255, 255, 255, 0.07);
            font-size: 11px;
            color: rgba(255, 255, 255, 0.55);
            cursor: pointer;
            user-select: none;
        }
        #${PROMPT_ID} .khp-foot:hover {
            color: rgba(255, 255, 255, 0.72);
        }
        #${PROMPT_ID} input[type="checkbox"] {
            margin: 0;
            accent-color: #10b981;
            width: 14px;
            height: 14px;
            cursor: pointer;
            flex-shrink: 0;
        }
        `;
        document.head.appendChild(style);
    }

    function removePrompt() {
        document.getElementById(PROMPT_ID)?.remove();
    }

    function escapeHtml(value) {
        return String(value ?? '')
            .replaceAll('&', '&amp;')
            .replaceAll('<', '&lt;')
            .replaceAll('>', '&gt;')
            .replaceAll('"', '&quot;')
            .replaceAll("'", '&#39;');
    }

    function showDownloadPrompt(payload) {
        ensurePromptStyle();
        removePrompt();
        const root = document.createElement('div');
        root.id = PROMPT_ID;
        const filename = payload.filename || '';
        const domain = payload.url ? (() => { try { return new URL(payload.url).hostname; } catch { return payload.url; } })() : '';
        const displayName = filename || domain || 'Unknown file';
        const displayTitle = filename || payload.url || domain || 'Unknown file';
        root.innerHTML = `
          <div class="khp-header">
            <div class="khp-icon">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
                <path d="M12 4v12m0 0l-4.5-4.5M12 16l4.5-4.5M5 19h14"
                  stroke="#34c759" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>
            <div class="khp-header-text">
              <div class="khp-title-row">
                <div class="khp-title">Ready to download</div>
                <span class="khp-badge">Khukri</span>
              </div>
              <div class="khp-sub" title="${escapeHtml(displayTitle)}">${escapeHtml(displayName)}</div>
            </div>
            <button class="khp-close" type="button" data-action="close" aria-label="Dismiss">
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>
              </svg>
            </button>
          </div>
          <div class="khp-body">
            <div class="khp-actions">
              <button class="khp-primary" type="button" data-action="start">
                <span class="khp-button-icon" aria-hidden="true">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                    <path d="M12 4v12m0 0l-4.5-4.5M12 16l4.5-4.5M5 19h14"
                      stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                </span>
                <span>Open in Khukri</span>
              </button>
            </div>
            <label class="khp-foot"><input type="checkbox" id="khukri-prompt-remember" /><span>Always use Khukri for downloads</span></label>
          </div>
        `;
        document.documentElement.appendChild(root);

        root.addEventListener('click', (event) => {
            const button = event.target.closest('button[data-action]');
            if (!button) return;
            const action = button.dataset.action === 'close' ? 'dismiss' : button.dataset.action;
            const remember = Boolean(root.querySelector('#khukri-prompt-remember')?.checked);
            safeSendMessage({
                type: 'khukri_prompt_decision',
                payload: {
                    ...payload,
                    action,
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

    function pagePlayerResponseMaxHeight() {
        const scripts = Array.from(document.scripts || []);
        let maxHeight = 0;
        for (const script of scripts) {
            const text = script.textContent || '';
            if (!text.includes('adaptiveFormats') && !text.includes('streamingData')) continue;
            const matches = text.matchAll(/"height"\s*:\s*(\d{3,4})/g);
            for (const match of matches) {
                maxHeight = Math.max(maxHeight, Number(match[1]) || 0);
            }
        }
        return maxHeight;
    }

    function currentVideoHeight() {
        const heights = Array.from(document.querySelectorAll('video'))
            .map((video) => Number(video.videoHeight || 0))
            .filter((height) => height > 0);
        return heights.length > 0 ? Math.max(...heights) : 0;
    }

    function detectedMaxHeight() {
        return Math.max(pagePlayerResponseMaxHeight(), currentVideoHeight());
    }

    function availableQualityOptions(maxHeight) {
        return QUALITY_OPTIONS.filter((option) => {
            const cap = QUALITY_HEIGHTS[option.value];
            return !cap || (maxHeight > 0 && cap <= maxHeight);
        });
    }

    function normalizeQualityForHeight(quality, maxHeight) {
        const available = availableQualityOptions(maxHeight);
        return available.some((option) => option.value === quality) ? quality : QUALITY_DEFAULT;
    }

    function maxHeightLabel(maxHeight) {
        return maxHeight ? `Up to ${maxHeight}p` : 'Detecting quality';
    }

    function renderQualityOptions(select, maxHeight, preferredQuality = select.value || QUALITY_DEFAULT) {
        const previous = normalizeQualityForHeight(preferredQuality, maxHeight);
        select.textContent = '';
        for (const option of availableQualityOptions(maxHeight)) {
            const node = document.createElement('option');
            node.value = option.value;
            node.textContent = option.label;
            select.appendChild(node);
        }
        select.value = previous;
        return previous;
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

    function subtitleText(quality, maxHeight) {
        if (quality === QUALITY_DEFAULT && maxHeight) {
            return `BEST AVAILABLE • ${maxHeight}P MAX`;
        }
        return subtitleForQuality(quality);
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
            let maxHeight = detectedMaxHeight();
            const selectedQuality = normalizeQualityForHeight(qualityForOrigin(result, origin), maxHeight);
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
            sub.textContent = subtitleText(activeQuality, maxHeight);
            const cap = document.createElement('div');
            cap.className = 'kh-cap';
            cap.textContent = maxHeightLabel(maxHeight);
            content.appendChild(kicker);
            content.appendChild(title);
            content.appendChild(sub);
            content.appendChild(cap);

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
            activeQuality = renderQualityOptions(qualitySelect, maxHeight, activeQuality);
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
                sub.textContent = subtitleText(activeQuality, maxHeight);
                saveQuality(origin, activeQuality);
            });

            const refreshDetectedQuality = () => {
                const nextMaxHeight = detectedMaxHeight();
                if (nextMaxHeight === maxHeight) return;
                maxHeight = nextMaxHeight;
                activeQuality = renderQualityOptions(qualitySelect, maxHeight, activeQuality);
                cap.textContent = maxHeightLabel(maxHeight);
                sub.textContent = subtitleText(activeQuality, maxHeight);
            };
            window.setTimeout(refreshDetectedQuality, 1200);
            document.querySelectorAll('video').forEach((video) => {
                video.addEventListener('loadedmetadata', refreshDetectedQuality, { once: true });
                video.addEventListener('resize', refreshDetectedQuality);
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
                if (event.target.closest('.kh-quality-wrap')) return;
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
        chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
            if (message?.type === 'khukri_prompt_download' && message.payload) {
                showDownloadPrompt(message.payload);
                sendResponse({ ok: true });
            }
        });
    }
})();
