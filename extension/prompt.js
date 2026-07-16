// Ask-mode download prompt. The payload itself stays in session storage and is
// addressed by the token in this page's query string.

'use strict';

let keepalivePort = null;
try {
    keepalivePort = chrome.runtime.connect({ name: 'khukri_prompt_keepalive' });
} catch (e) {
    console.warn('Khukri prompt: could not open keepalive port:', e.message);
}

const PROMPT_MAX_AGE_MS = 5 * 60 * 1000; // 5 minutes — after this, auto-keep

// Helpers

function getToken() {
    try {
        const params = new URLSearchParams(window.location.search);
        return params.get('token') || '';
    } catch {
        return '';
    }
}

function formatBytes(bytes) {
    if (!bytes || bytes <= 0) return 'Unknown size';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function truncateUrl(url, maxLen = 52) {
    if (!url) return '';
    try {
        const u = new URL(url);
        const short = u.hostname + u.pathname;
        return short.length > maxLen ? short.slice(0, maxLen) + '…' : short;
    } catch {
        return url.length > maxLen ? url.slice(0, maxLen) + '…' : url;
    }
}

// Service worker communication

async function loadPayload(token) {
    let payload;
    try {
        payload = await chrome.runtime.sendMessage({ type: 'khukri_prompt_get', token });
    } catch (e) {
        console.warn('Khukri prompt: SW unreachable on load:', e.message);
        return null;
    }

    if (!payload) return null;

    // Stale prompts should not retain a cancelled browser download indefinitely.
    if (Date.now() - payload.createdAt > PROMPT_MAX_AGE_MS) {
        console.warn('Khukri prompt: payload too old, auto-keeping in browser');
        await sendDecision(token, 'keep', false);
        return null;
    }

    return payload;
}

async function sendDecision(token, action, remember) {
    try {
        const result = await chrome.runtime.sendMessage({
            type: 'khukri_prompt_choose',
            token,
            action,
            remember
        });
        return result?.ok === true;
    } catch (e) {
        console.warn('Khukri prompt: SW unreachable on decision:', e.message);
        return false;
    } finally {
        closeSelf();
    }
}

function closeSelf() {
    try {
        if (chrome.tabs?.getCurrent) {
            chrome.tabs.getCurrent((tab) => {
                if (chrome.runtime.lastError) {
                    window.close();
                    return;
                }
                if (tab?.id) {
                    chrome.tabs.remove(tab.id, () => window.close());
                    return;
                }
                window.close();
            });
            return;
        }
    } catch {
        // Fall through to window.close().
    }
    window.close();
}

// UI rendering

function showError(message) {
    const app = document.getElementById('app');
    if (!app) return;
    app.innerHTML = `
        <div class="error-state">
            <span class="error-icon">⚠</span>
            <p class="error-msg">${message}</p>
            <button class="btn btn-secondary" id="closeBtn">Close</button>
        </div>
    `;
    document.getElementById('closeBtn')?.addEventListener('click', () => closeSelf());
}

function renderPrompt(payload, token) {
    const filenameEl = document.getElementById('filename');
    const sizeEl = document.getElementById('filesize');
    const urlEl = document.getElementById('fileurl');
    const startBtn = document.getElementById('startBtn');
    const keepBtn = document.getElementById('keepBtn');
    const rememberChk = document.getElementById('rememberChk');

    if (filenameEl) filenameEl.textContent = payload.filename || 'Unknown file';
    if (sizeEl) sizeEl.textContent = formatBytes(payload.size);
    if (urlEl) {
        urlEl.textContent = truncateUrl(payload.url);
        urlEl.title = payload.url;
    }

    startBtn?.addEventListener('click', async () => {
        startBtn.disabled = true;
        keepBtn.disabled = true;
        startBtn.textContent = 'Starting…';
        const remember = rememberChk?.checked ?? false;
        await sendDecision(token, 'start', remember);
    });

    keepBtn?.addEventListener('click', async () => {
        keepBtn.disabled = true;
        startBtn.disabled = true;
        keepBtn.textContent = 'Keeping…';
        const remember = rememberChk?.checked ?? false;
        await sendDecision(token, 'keep', remember);
    });
}

// Boot

document.addEventListener('DOMContentLoaded', async () => {
    const token = getToken();

    if (!token) {
        showError('Missing download token. Please close this window.');
        return;
    }

    if (!keepalivePort) {
        console.warn('Khukri prompt: running without keepalive port (SW may sleep)');
    }

    const payload = await loadPayload(token);

    if (!payload) {
        showError('Download info unavailable. The download will proceed in the browser.');
        setTimeout(() => closeSelf(), 2000);
        return;
    }

    renderPrompt(payload, token);
});
