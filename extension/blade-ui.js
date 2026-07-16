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
                stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>`;

    const ICON_CLOSE = `
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M18 6L6 18M6 6l12 12"
                stroke="currentColor" stroke-width="2.3" stroke-linecap="round"/>
        </svg>`;

    const PILL_STYLE = `
        @keyframes khukri-in {
            from { opacity: 0; transform: translateY(-8px) scale(0.97); }
            to   { opacity: 1; transform: translateY(0) scale(1); }
        }
        @keyframes khukri-out {
            from { opacity: 1; transform: translateY(0) scale(1); }
            to   { opacity: 0; transform: translateY(-6px) scale(0.95); }
        }

        #${PILL_ID} {
            --kh-bg: #1c1917;
            --kh-surface: #292524;
            --kh-surface-hi: #44403c;
            --kh-text: #fafaf9;
            --kh-text-soft: #d6d3d1;
            --kh-text-muted: #a8a29e;
            --kh-accent: #818cf8;
            --kh-accent-hover: #a5b4fc;
            --kh-accent-bg: rgba(99, 102, 241, 0.16);
            --kh-border: rgba(255, 255, 255, 0.09);
            --kh-border-strong: rgba(255, 255, 255, 0.15);
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
            background: rgba(41, 37, 36, 0.9);
            border: 1px solid var(--kh-border);
            box-shadow:
                0 14px 38px rgba(0, 0, 0, 0.42),
                inset 0 1px 0 rgba(255, 255, 255, 0.055),
                0 0 28px rgba(99, 102, 241, 0.1);
            color: var(--kh-text);
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
            user-select: none;
            outline: none;
            overflow: hidden;
            backdrop-filter: blur(24px) saturate(150%);
            -webkit-backdrop-filter: blur(24px) saturate(150%);
            animation: khukri-in 280ms cubic-bezier(0.16, 1, 0.3, 1) both;
            transition: box-shadow 180ms ease, border-color 180ms ease, transform 150ms ease;
        }

        #${PILL_ID}:hover {
            border-color: var(--kh-border-strong);
            transform: translateY(-1px);
            box-shadow:
                0 18px 46px rgba(0, 0, 0, 0.5),
                inset 0 1px 0 rgba(255, 255, 255, 0.07),
                0 0 34px rgba(99, 102, 241, 0.16);
        }

        #${PILL_ID}:focus-visible {
            outline: 3px solid rgba(129, 140, 248, 0.42);
            outline-offset: 3px;
        }

        #${PILL_ID} .kh-main {
            display: flex;
            align-items: center;
            gap: 0;
            position: relative;
            z-index: 1;
        }

        #${PILL_ID} .kh-icon-zone {
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 10px 11px;
            background: linear-gradient(180deg, rgba(99,102,241,0.2), rgba(99,102,241,0.08));
            border-right: 1px solid rgba(129, 140, 248, 0.18);
        }

        #${PILL_ID} .kh-icon-circle {
            width: 30px;
            height: 30px;
            border-radius: 8px;
            background: var(--kh-accent-bg);
            border: 1px solid rgba(129, 140, 248, 0.3);
            display: flex;
            align-items: center;
            justify-content: center;
            color: var(--kh-accent-hover);
            box-shadow: 0 7px 16px rgba(99, 102, 241, 0.18);
        }

        #${PILL_ID} .kh-icon-circle svg {
            width: 14px;
            height: 14px;
        }

        #${PILL_ID} .kh-content {
            display: flex;
            flex-direction: column;
            justify-content: center;
            min-width: 0;
            padding: 9px 13px;
        }

        #${PILL_ID} .kh-kicker {
            display: block;
            font-size: 9px;
            font-weight: 700;
            letter-spacing: 0.12em;
            text-transform: uppercase;
            color: var(--kh-accent-hover);
            margin-bottom: 2px;
        }

        #${PILL_ID} .kh-sub { display: none; }

        #${PILL_ID} .kh-title {
            font-size: 13px;
            font-weight: 600;
            line-height: 1.15;
            color: var(--kh-text);
            white-space: nowrap;
            letter-spacing: -0.01em;
            display: flex;
            align-items: center;
            gap: 4px;
        }

        #${PILL_ID} .kh-brand {
            color: var(--kh-accent-hover);
            font-weight: 700;
        }

        #${PILL_ID} .kh-cap { display: none; }

        #${PILL_ID} .kh-quality-wrap {
            display: flex;
            align-items: center;
            padding-right: 4px;
        }

        #${PILL_ID} .kh-quality-label { display: none; }

        #${PILL_ID} .kh-quality-select {
            width: auto;
            border: 1px solid var(--kh-border);
            border-radius: 8px;
            background: rgba(255, 255, 255, 0.055);
            color: var(--kh-text-soft);
            font-size: 11px;
            font-weight: 600;
            height: 32px;
            padding: 0 24px 0 10px;
            outline: none;
            cursor: pointer;
            transition: background-color 180ms ease, border-color 180ms ease, color 180ms ease;
            appearance: none;
            -webkit-appearance: none;
            color-scheme: dark;
            background-image: url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='white' stroke-opacity='0.5' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
            background-repeat: no-repeat;
            background-position: right 8px center;
        }

        #${PILL_ID} .kh-quality-select:hover {
            background-color: rgba(255, 255, 255, 0.09);
            border-color: var(--kh-border-strong);
            color: var(--kh-text);
        }

        #${PILL_ID} .kh-quality-select:focus-visible,
        #${PILL_ID} .kh-close:focus-visible {
            outline: 3px solid rgba(129, 140, 248, 0.38);
            outline-offset: 2px;
        }

        #${PILL_ID} .kh-close {
            display: flex;
            align-items: center;
            justify-content: center;
            width: 36px;
            height: 36px;
            margin: 7px 7px 7px 1px;
            border-radius: 8px;
            background: transparent;
            border: 1px solid transparent;
            cursor: pointer;
            color: var(--kh-text-muted);
            transition: color 180ms ease, background-color 180ms ease, border-color 180ms ease;
            flex-shrink: 0;
        }

        #${PILL_ID} .kh-close:hover {
            color: #f87171;
            background: rgba(248, 113, 113, 0.12);
            border-color: rgba(248, 113, 113, 0.18);
        }

        #${PILL_ID}.kh-dismissing {
            animation: khukri-out 0.2s cubic-bezier(0.4, 0, 1, 1) both !important;
            pointer-events: none;
        }

        @media (max-width: 960px) {
            #${PILL_ID} { top: 12px; right: 12px; }
        }

        @media (prefers-reduced-motion: reduce) {
            #${PILL_ID}, #${PILL_ID} * {
                animation-duration: 0.01ms !important;
                transition-duration: 0.01ms !important;
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

    function sendMessageWithResponse(message) {
        if (!hasExtensionContext()) {
            return Promise.resolve({ ok: false, error: 'The Khukri extension needs to be reloaded.' });
        }
        return new Promise((resolve) => {
            try {
                chrome.runtime.sendMessage(message, (response) => {
                    const error = chrome.runtime?.lastError?.message;
                    if (error) {
                        resolve({ ok: false, error });
                        return;
                    }
                    resolve(response || { ok: false, error: 'Khukri did not respond.' });
                });
            } catch (error) {
                resolve({ ok: false, error: error?.message || 'Khukri did not respond.' });
            }
        });
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
            --kh-bg: #1c1917;
            --kh-surface: #292524;
            --kh-surface-hi: #44403c;
            --kh-text: #fafaf9;
            --kh-text-soft: #d6d3d1;
            --kh-text-muted: #a8a29e;
            --kh-accent: #818cf8;
            --kh-accent-hover: #a5b4fc;
            --kh-accent-bg: rgba(99, 102, 241, 0.16);
            --kh-border: rgba(255, 255, 255, 0.09);
            --kh-border-strong: rgba(255, 255, 255, 0.15);
            position: fixed;
            right: 18px;
            bottom: 18px;
            z-index: 2147483647;
            width: min(390px, calc(100vw - 24px));
            background:
                radial-gradient(circle at 85% 0%, rgba(99, 102, 241, 0.15), transparent 45%),
                rgba(41, 37, 36, 0.94);
            border: 1px solid var(--kh-border);
            border-radius: 16px;
            box-shadow:
                0 20px 54px rgba(0, 0, 0, 0.44),
                inset 0 1px 0 rgba(255, 255, 255, 0.055),
                0 0 34px rgba(99, 102, 241, 0.1);
            color: var(--kh-text);
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
            overflow: hidden;
            backdrop-filter: blur(24px) saturate(150%);
            -webkit-backdrop-filter: blur(24px) saturate(150%);
            animation: khukri-prompt-in 280ms cubic-bezier(0.16, 1, 0.3, 1) both;
        }
        #${PROMPT_ID} .khp-header {
            display: flex;
            align-items: center;
            gap: 11px;
            padding: 14px 14px 12px 15px;
            border-bottom: 1px solid var(--kh-border);
        }
        #${PROMPT_ID} .khp-icon {
            width: 30px;
            height: 30px;
            border-radius: 9px;
            background: var(--kh-accent-bg);
            border: 1px solid rgba(129, 140, 248, 0.28);
            color: var(--kh-accent-hover);
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
            width: 32px;
            height: 32px;
            border-radius: 8px;
            background: transparent;
            border: 1px solid transparent;
            cursor: pointer;
            color: var(--kh-text-muted);
            transition: color 180ms ease, background-color 180ms ease, border-color 180ms ease;
            flex-shrink: 0;
            padding: 0;
            min-height: 32px;
        }
        #${PROMPT_ID} .khp-close:hover {
            color: #f87171;
            background: rgba(248,113,113,0.12);
            border-color: rgba(248,113,113,0.18);
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
            color: var(--kh-text);
            line-height: 1.2;
            white-space: nowrap;
        }
        #${PROMPT_ID} .khp-badge {
            display: inline-flex;
            align-items: center;
            height: 18px;
            padding: 0 7px;
            border-radius: 999px;
            background: var(--kh-accent-bg);
            border: 1px solid rgba(129, 140, 248, 0.24);
            color: var(--kh-accent-hover);
            font-size: 10px;
            font-weight: 700;
            line-height: 1;
            flex-shrink: 0;
        }
        #${PROMPT_ID} .khp-sub {
            max-width: 300px;
            font-size: 11.5px;
            color: var(--kh-text-muted);
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
            min-height: 44px;
            padding: 10px 13px;
            cursor: pointer;
            font-size: 12.5px;
            font-weight: 700;
            color: var(--kh-text-soft);
            background: rgba(255, 255, 255, 0.055);
            transition: background-color 180ms ease, color 180ms ease, transform 140ms ease, box-shadow 180ms ease;
            line-height: 1;
            white-space: nowrap;
        }
        #${PROMPT_ID} button:hover {
            background: rgba(255, 255, 255, 0.09);
            color: var(--kh-text);
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
            background: var(--kh-accent);
            color: #17151f;
            box-shadow: 0 9px 24px rgba(99, 102, 241, 0.24);
        }
        #${PROMPT_ID} .khp-primary:hover {
            background: var(--kh-accent-hover);
            color: #17151f;
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
            border-top: 1px solid var(--kh-border);
            font-size: 11.5px;
            color: var(--kh-text-muted);
            cursor: pointer;
            user-select: none;
        }
        #${PROMPT_ID} .khp-foot:hover {
            color: var(--kh-text-soft);
        }
        #${PROMPT_ID} input[type="checkbox"] {
            margin: 0;
            accent-color: var(--kh-accent);
            width: 16px;
            height: 16px;
            cursor: pointer;
            flex-shrink: 0;
        }
        #${PROMPT_ID} .khp-error {
            display: none;
            margin-top: 9px;
            color: #f87171;
            font-size: 11px;
            line-height: 1.35;
        }
        #${PROMPT_ID} .khp-error[data-visible="true"] { display: block; }
        #${PROMPT_ID} button:focus-visible,
        #${PROMPT_ID} input:focus-visible {
            outline: 3px solid rgba(129, 140, 248, 0.4);
            outline-offset: 2px;
        }
        #${PROMPT_ID} button:disabled {
            opacity: 0.48;
            cursor: not-allowed;
            transform: none;
        }
        @media (prefers-reduced-motion: reduce) {
            #${PROMPT_ID}, #${PROMPT_ID} * {
                animation-duration: 0.01ms !important;
                transition-duration: 0.01ms !important;
            }
        }
        `;
        document.head.appendChild(style);
    }

    function removePrompt() {
        document.getElementById(PROMPT_ID)?.remove();
    }

    function promptQualityLabel(quality) {
        const labels = {
            best: 'Best quality',
            '1080p': 'Up to 1080p',
            '720p': 'Up to 720p',
            'audio-only': 'Audio only'
        };
        return labels[quality] || 'Ready';
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
        clearPill();
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
                  stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>
            <div class="khp-header-text">
              <div class="khp-title-row">
                <div class="khp-title">Ready to download</div>
                <span class="khp-badge">${escapeHtml(promptQualityLabel(payload.quality))}</span>
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
            <div class="khp-error" role="alert"></div>
            <label class="khp-foot"><input type="checkbox" id="khukri-prompt-remember" /><span>Always use Khukri for downloads</span></label>
          </div>
        `;
        document.documentElement.appendChild(root);

        root.addEventListener('click', async (event) => {
            const button = event.target.closest('button[data-action]');
            if (!button) return;
            const action = button.dataset.action === 'close' ? 'dismiss' : button.dataset.action;
            const remember = Boolean(root.querySelector('#khukri-prompt-remember')?.checked);
            if (action === 'dismiss') {
                safeSendMessage({
                    type: 'khukri_prompt_decision',
                    payload: { ...payload, action, remember }
                });
                removePrompt();
                return;
            }

            const label = button.querySelector('span:last-child');
            const errorNode = root.querySelector('.khp-error');
            button.disabled = true;
            if (label) label.textContent = 'Connecting…';
            if (errorNode) errorNode.dataset.visible = 'false';

            const response = await sendMessageWithResponse({
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
            if (response?.ok) {
                removePrompt();
                return;
            }

            button.disabled = false;
            if (label) label.textContent = 'Try again';
            if (errorNode) {
                errorNode.textContent = response?.error || 'Could not connect to Khukri.';
                errorNode.dataset.visible = 'true';
            }
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

        if (document.getElementById(PROMPT_ID)) return;

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
            title.appendChild(document.createTextNode('Download with '));
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
        if (document.getElementById(PROMPT_ID)) return;
        showTimer = window.setTimeout(() => {
            showTimer = null;
            if (document.getElementById(PROMPT_ID)) return;
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
        if (!document.getElementById(PILL_ID)
            && !document.getElementById(PROMPT_ID)
            && document.querySelector('video')) {
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
