// =============================================================================
// Khukri Extension — prompt.js
// Download interception prompt popup script.
//
// This script runs inside prompt.html, which is opened as a chrome popup
// window by the service worker when intercept_mode is 'ask'.
//
// Flow:
//   1. Parse ?token= from the URL
//   2. Open keepalive port so the SW stays alive while user reads the dialog
//   3. Load the download payload from SW via khukri_prompt_get
//   4. Render filename, size, URL into the UI
//   5. User clicks "Start in Khukri" or "Keep in Browser"
//   6. Send khukri_prompt_choose to SW with the decision
//   7. Close the popup
// =============================================================================

'use strict';

// ─────────────────────────────────────────────────────────────────────────────
// FIX 3 — Keepalive port
// Open immediately, before anything else. While this port is alive the service
// worker cannot be suspended by Chrome. This means the user can take their
// time reading the dialog and clicking — the SW will still be there.
// ─────────────────────────────────────────────────────────────────────────────
let _keepalivePort = null;
try {
    _keepalivePort = chrome.runtime.connect({ name: 'khukri_prompt_keepalive' });
} catch (e) {
    // Extension was reloaded between popup open and script execution.
    // Nothing we can do — the popup will show an error state below.
    console.warn('Khukri prompt: could not open keepalive port:', e.message);
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────
const PROMPT_MAX_AGE_MS = 5 * 60 * 1000; // 5 minutes — after this, auto-keep

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// SW communication
// ─────────────────────────────────────────────────────────────────────────────

async function loadPayload(token) {
    let payload;
    try {
        payload = await chrome.runtime.sendMessage({ type: 'khukri_prompt_get', token });
    } catch (e) {
        console.warn('Khukri prompt: SW unreachable on load:', e.message);
        return null;
    }

    if (!payload) return null;

    // TTL guard — if popup was somehow left open for too long, auto-keep
    if (Date.now() - payload.createdAt > PROMPT_MAX_AGE_MS) {
        console.warn('Khukri prompt: payload too old, auto-keeping in browser');
        await sendDecision(token, 'keep', false);
        return null; // sendDecision closes the window
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
        // SW died between popup open and button click. The download was already
        // cancelled; the SW retry queue will recover it on next wake.
        console.warn('Khukri prompt: SW unreachable on decision:', e.message);
        return false;
    } finally {
        window.close();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UI rendering
// ─────────────────────────────────────────────────────────────────────────────

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
    document.getElementById('closeBtn')?.addEventListener('click', () => window.close());
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
        urlEl.title = payload.url; // full URL on hover
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

// ─────────────────────────────────────────────────────────────────────────────
// Boot
// ─────────────────────────────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', async () => {
    const token = getToken();

    if (!token) {
        showError('Missing download token. Please close this window.');
        return;
    }

    if (!_keepalivePort) {
        // Keepalive failed — extension was probably reloaded. Still try to work.
        console.warn('Khukri prompt: running without keepalive port (SW may sleep)');
    }

    const payload = await loadPayload(token);

    if (!payload) {
        // loadPayload already handled the fallback (auto-keep or error)
        // sendDecision closes the window; if it didn't (null payload from SW
        // being gone), close manually after a short message.
        showError('Download info unavailable. The download will proceed in the browser.');
        setTimeout(() => window.close(), 2000);
        return;
    }

    renderPrompt(payload, token);
});