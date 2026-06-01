const FALLBACK_STRINGS = {
  "app.kicker": "Khukri",
  "app.title": "Khukri",
  "app.subtitle": "Fast, local-first downloads with a clean queue and no noise.",
  "nav.downloads": "All Downloads",
  "nav.settings": "Settings",
  "hero.eyebrow": "Downloads",
  "hero.title": "Drop in a link. Khukri handles the rest.",
  "hero.copy": "Direct files and media links share one queue, one history, and one place to recover when the network misbehaves.",
  "composer.title": "New download",
  "composer.copy": "Paste a direct file or media link and choose where it should land.",
  "composer.urlLabel": "Download URL",
  "composer.outputLabel": "Output path",
  "composer.priorityLabel": "Priority",
  "composer.qualityLabel": "Mode",
  "composer.throttleLabel": "Throttle bytes/sec",
  "priority.high": "High",
  "priority.normal": "Normal",
  "priority.low": "Low",
  "quality.direct": "Direct file",
  "quality.best": "Best video",
  "quality.1080p": "1080p",
  "quality.720p": "720p",
  "quality.audioOnly": "Audio only",
  "actions.start": "Start Download",
  "actions.refresh": "Refresh Queue",
  "actions.new_download": "New Download",
  "actions.remove": "Remove",
  "downloads.title": "Current queue",
  "downloads.copy": "Every transfer lands in one durable queue.",
  "downloads.emptyTitle": "No downloads yet",
  "downloads.emptyCopy": "Use New Download or the browser extension. Active, failed, and completed items will stay organized here.",
  "downloads.priorityValue": "Priority",
  "downloads.totalBytesValue": "Total bytes",
  "downloads.unknownBytes": "unknown",
  "downloads.speedValue": "Speed",
  "downloads.etaValue": "ETA",
  "downloads.progressValue": "Progress",
  "downloads.actionOpen": "Open Folder",
  "downloads.actionPause": "Pause",
  "downloads.actionResume": "Resume",
  "downloads.actionCancel": "Cancel",
  "downloads.failureLabel": "Failure",
  "downloads.fileMissing": "File is no longer on disk. Remove this stale entry.",
  "status.ready": "Ready",
  "status.loading": "Loading queue...",
  "status.started": "Download queued.",
  "status.failed": "Something went wrong. Check the Rust console for details.",
  "settings.title": "Settings",
  "settings.copy": "Tune where files land, how Khukri uses bandwidth, and how media helpers behave.",
  "settings.reset": "Reset to defaults",
  "settings.save": "Save Settings",
  "settings.saved": "Settings saved.",
  "settings.unsaved": "Unsaved changes",
  "settings.general.title": "General",
  "settings.general.copy": "Choose where downloads land and how many can run at once.",
  "settings.general.pathLabel": "Default download path",
  "settings.general.pathHint": "Where Khukri stores new downloads by default.",
  "settings.general.concurrentLabel": "Max concurrent downloads",
  "settings.general.concurrentHint": "Choose how many downloads can run at the same time.",
  "settings.general.ytdlpAutoUpdateLabel": "Automatically check for yt-dlp updates",
  "settings.general.ytdlpActionsLabel": "yt-dlp",
  "settings.general.ytdlpCheckNow": "Check now",
  "settings.performance.title": "Performance",
  "settings.performance.copy": "Set per-download overrides that new downloads will inherit.",
  "settings.performance.threadsLabel": "Worker threads",
  "settings.performance.threadsHint": "Leave blank to let Khukri choose automatically.",
  "settings.performance.bandwidthLabel": "Speed limit",
  "settings.performance.bandwidthHint": "Optional. Leave blank for unlimited transfer speed.",
  "settings.scheduler.title": "Scheduler",
  "settings.scheduler.copy": "Reserve a time window for automated downloads.",
  "settings.scheduler.enabledLabel": "Enable scheduler window",
  "settings.scheduler.startLabel": "Start hour",
  "settings.scheduler.endLabel": "End hour",
  "settings.scheduler.hint": "Queued downloads outside this window will wait until the schedule opens again.",
  "settings.proxy.title": "Proxy",
  "settings.proxy.copy": "Route downloads through a proxy when your network needs one.",
  "settings.proxy.enabledLabel": "Enable proxy",
  "settings.proxy.urlLabel": "Proxy URL",
  "settings.proxy.urlHint": "Use only when your network requires a proxy or VPN route.",
  "settings.appearance.title": "Appearance",
  "settings.appearance.copy": "Choose how the desktop shell should resolve its theme.",
  "settings.appearance.themeLabel": "Theme",
  "settings.appearance.themeSystem": "Follow system",
  "settings.appearance.themeDark": "Dark",
  "settings.appearance.themeLight": "Light",
  "settings.appearance.themeHint": "Match your system appearance automatically, or choose a fixed theme.",
  "onboarding.kicker": "Media notice",
  "onboarding.title": "Media tools require a one-time acknowledgment.",
  "onboarding.body": "yt-dlp functionality is provided as a technical capability. Compliance with the Terms of Service of any streaming platform is the user's responsibility. Khukri ships no credentials, no DRM bypass, and no circumvention of technical protection measures.",
  "onboarding.accept": "I Understand",
  "ytdlp.updateStarted": "Checking for yt-dlp updates...",
  "ytdlp.updateApplied": "yt-dlp update applied.",
  "ytdlp.updateNoop": "yt-dlp is already current."
};

const progressById = new Map();
const pendingActions = new Set();
let currentQueue = [];
let currentSettings = null;
let currentView = "downloads";
let queueRefreshInFlight = false;
let settingsDirty = false;
let renderQueueFrame = null;

function onboardingComplete(settings) {
  return Boolean(settings?.onboarding_complete);
}

function toggleOnboarding(settings) {
  const overlay = document.getElementById("mediaOnboarding");
  if (!overlay) {
    return;
  }

  const shouldShow = !onboardingComplete(settings);
  overlay.hidden = !shouldShow;
  document.body.classList.toggle("onboarding-open", shouldShow);

  if (shouldShow) {
    document.getElementById("acknowledgeOnboarding")?.focus();
  }
}

function persistThemePreference(themeMode) {
  try {
    localStorage.setItem("khukri-theme", themeMode || "system");
  } catch {
    // Ignore storage failures; theme still applies for the current session.
  }
}

async function loadStrings() {
  try {
    const response = await fetch("./i18n/en.json");
    if (!response.ok) {
      return FALLBACK_STRINGS;
    }
    return await response.json();
  } catch {
    return FALLBACK_STRINGS;
  }
}

function applyStrings(strings) {
  document.querySelectorAll("[data-i18n]").forEach((node) => {
    const key = node.getAttribute("data-i18n");
    node.textContent = strings[key] || FALLBACK_STRINGS[key] || key;
  });
}

function errorText(error) {
  if (typeof error === "string") {
    return error;
  }
  if (error && typeof error.message === "string" && error.message.trim()) {
    return error.message;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function invoke(command, payload = {}) {
  const api = window.__TAURI__?.core;
  if (!api?.invoke) {
    return Promise.reject(new Error("Tauri invoke API is unavailable."));
  }
  return api.invoke(command, payload);
}

function getCurrentWindowHandle() {
  return window.__TAURI__?.window?.getCurrentWindow?.() ?? null;
}

function formatBytes(value) {
  if (value == null || Number.isNaN(value)) {
    return window.__KHUKRI_STRINGS__["downloads.unknownBytes"];
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = Number(value);
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  const precision = unitIndex === 0 ? 0 : 1;
  return `${size.toFixed(precision)} ${units[unitIndex]}`;
}

function formatEta(seconds) {
  if (seconds == null || seconds <= 0) {
    return "—";
  }

  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  if (mins === 0) {
    return `${secs}s`;
  }
  return `${mins}m ${secs}s`;
}

function baseName(path) {
  return path.split(/[/\\]/).pop() || path;
}

function fileExtInfo(filePath) {
  const name = baseName(filePath);
  const dot = name.lastIndexOf(".");
  if (dot < 0) return { ext: "bin", category: "other" };
  const ext = name.slice(dot + 1).toLowerCase().slice(0, 4);
  const video     = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];
  const audio     = ["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus"];
  const image     = ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico"];
  const archive   = ["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "zst"];
  const doc       = ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "txt", "csv"];
  const installer = ["exe", "msi", "deb", "apk", "dmg", "pkg", "rpm"];
  if (video.includes(ext))     return { ext, category: "video" };
  if (audio.includes(ext))     return { ext, category: "audio" };
  if (image.includes(ext))     return { ext, category: "image" };
  if (archive.includes(ext))   return { ext, category: "archive" };
  if (doc.includes(ext))       return { ext, category: "doc" };
  if (installer.includes(ext)) return { ext, category: "installer" };
  return { ext: ext || "bin", category: "other" };
}

function setStatusReady(node, label) {
  node.innerHTML = `<span class="status-dot" aria-hidden="true"></span>${htmlEscape(label)}`;
}

function htmlEscape(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#39;");
}

function safeStatus(value) {
  const status = String(value ?? "");
  return ["queued", "active", "paused", "complete", "failed", "cancelled", "missing"].includes(status)
    ? status
    : "queued";
}

function rowPrimaryAction(item) {
  const status = item.liveStatus ?? item.status;
  if (status === "complete") {
    return item.fileExists === false ? "remove" : "open";
  }
  if (status === "paused") {
    return "resume";
  }
  if (status === "failed") {
    return "retry";
  }
  if (status === "cancelled" || status === "missing") {
    return "remove";
  }
  return "pause";
}

function mergeQueueWithProgress(items) {
  return items.map((item) => {
    const progress = progressById.get(item.id);
    const fileMissing = item.status === "complete" && item.fileExists === false;
    const hasLiveActiveProgress = progress && ["queued", "active"].includes(progress.status);
    const backendTerminal = ["failed", "complete", "cancelled"].includes(item.status)
      || (item.status === "paused" && !hasLiveActiveProgress);
    const liveStatus = fileMissing
      ? "missing"
      : (hasLiveActiveProgress ? progress.status : (backendTerminal ? item.status : (progress?.status ?? item.status)));
    const totalBytes = progress?.totalBytes ?? item.totalBytes ?? null;
    const bytesDone = (liveStatus === "complete" || liveStatus === "missing") && totalBytes != null
      ? Number(totalBytes)
      : progress?.bytesDone ?? 0;
    const percent = totalBytes && totalBytes > 0
      ? Math.max(0, Math.min(100, (bytesDone / totalBytes) * 100))
      : 0;

    return {
      ...item,
      liveStatus,
      bytesDone,
      totalBytes,
      speedBps: backendTerminal ? 0 : (progress?.speedBps ?? 0),
      etaSeconds: backendTerminal ? null : (progress?.etaSeconds ?? null),
      percent: liveStatus === "complete" || liveStatus === "missing" ? 100 : percent
    };
  });
}

function updateLocalDownloadState(id, updater) {
  currentQueue = currentQueue.map((entry) => {
    if (entry.id !== id) {
      return entry;
    }
    return updater(entry);
  });
}

function setOptimisticProgress(id, status) {
  const previous = progressById.get(id);
  if (!previous) {
    return;
  }

  progressById.set(id, {
    ...previous,
    status,
    speedBps: status === "active" ? previous.speedBps : 0,
    etaSeconds: status === "active" ? previous.etaSeconds : null
  });
}

function queueGroupForStatus(status) {
  if (["active", "queued", "paused"].includes(status)) {
    return "active";
  }
  if (status === "complete") {
    return "complete";
  }
  return "failed";
}

function sectionHeading(group) {
  if (group === "active") return "Active";
  if (group === "failed") return "Failed";
  return "Completed";
}

function statusLabel(status) {
  const labels = {
    active: "Downloading",
    queued: "Queued",
    paused: "Paused",
    complete: "Done",
    failed: "Failed",
    cancelled: "Cancelled",
    missing: "Missing"
  };
  return labels[status] || status;
}

function qualityLabel(value) {
  const labels = {
    best: "Best",
    "1080p": "1080p",
    "720p": "720p",
    "audio-only": "Audio"
  };
  return labels[value] || "Direct";
}

function sourceLabel(value) {
  const labels = {
    blade: "Blade",
    browser: "Browser",
    stream: "Stream"
  };
  return labels[value] || "App";
}

function renderMetaChips(item, liveStatus) {
  const chips = [
    statusLabel(liveStatus),
    qualityLabel(item.mediaQuality),
    sourceLabel(item.requestSource)
  ];

  if (item.totalBytes) {
    chips.push(formatBytes(item.totalBytes));
  }

  return `
    <div class="queue-card-chips" aria-label="Download details">
      ${chips.map((chip) => `<span class="queue-chip">${htmlEscape(chip)}</span>`).join("")}
    </div>
  `;
}

function formatProgressSummary(item) {
  const parts = [];
  const percent = `${Math.max(0, item.percent).toFixed(0)}%`;
  parts.push(percent);
  if (item.speedBps > 0) {
    parts.push(`${formatBytes(item.speedBps)}/s`);
  }
  if (item.etaSeconds > 0) {
    parts.push(`~${formatEta(item.etaSeconds)} remaining`);
  }
  if (item.totalBytes) {
    const done = item.liveStatus === "complete"
      ? item.totalBytes
      : Math.min(item.bytesDone ?? 0, item.totalBytes);
    parts.push(`${formatBytes(done)} / ${formatBytes(item.totalBytes)}`);
  }
  if (!item.totalBytes && ["active", "queued"].includes(item.liveStatus)) {
    parts.push("Waiting for size");
  }
  return parts.join(" • ");
}

function summarizeFailure(item) {
  const raw = item.failureReason || "";
  if (/sign in to confirm you('|’)re not a bot|cookies-from-browser|authentication/i.test(raw)) {
    return {
      title: "Sign-in required",
      body: "YouTube challenged this request. Open the video in your browser, make sure playback works there, then retry in Khukri.",
      detail: raw
    };
  }

  if (/rate limit|too many requests|429/i.test(raw)) {
    return {
      title: "Temporarily rate limited",
      body: "The site is throttling repeated requests right now. Waiting a bit before retrying usually helps.",
      detail: raw
    };
  }

  return {
    title: "Download failed",
    body: "Khukri could not complete this download. You can retry it or remove it from the queue.",
    detail: raw
  };
}

function buildRetryRequest(item) {
  return {
    url: item.url,
    filePath: item.filePath,
    priority: item.priority || "normal",
    bytesPerSec: item.throttleBytesPerSec ?? null,
    quality: item.mediaQuality || null,
    source: item.requestSource || null
  };
}

function renderDownloadCard(item, index, strings) {
  const liveStatus = safeStatus(item.liveStatus ?? item.status);
  const group = queueGroupForStatus(liveStatus);
  const isPending = pendingActions.has(item.id);
  const pendingAttrs = isPending ? " disabled aria-disabled=\"true\"" : "";
  const percent = liveStatus === "complete" || liveStatus === "missing" ? 100 : item.percent;
  const isIndeterminate = ["queued", "active"].includes(liveStatus)
    && (item.bytesDone ?? 0) <= 0
    && (!item.totalBytes || item.percent <= 0);
  const progressFillClass = isIndeterminate
    ? `progress-fill progress-fill--${liveStatus} progress-fill--indeterminate`
    : `progress-fill progress-fill--${liveStatus}`;
  const { ext, category } = fileExtInfo(item.filePath);
  const escapedId = htmlEscape(item.id);
  const fileName = baseName(item.filePath);
  const escapedName = htmlEscape(fileName);
  const escapedUrl = htmlEscape(item.url);
  const pillLabel = statusLabel(liveStatus);

  let actions = "";
  let extra = "";

  if (group === "active") {
    const primaryAction = liveStatus === "paused" ? "resume" : "pause";
    const primaryLabel = liveStatus === "paused"
      ? strings["downloads.actionResume"]
      : strings["downloads.actionPause"];
    const summary = formatProgressSummary(item);

    actions = `
      <div class="queue-card-actions">
        <button class="queue-btn queue-btn-secondary row-action" type="button" data-action="${htmlEscape(primaryAction)}" data-id="${escapedId}"${pendingAttrs}>${htmlEscape(primaryLabel)}</button>
        <button class="queue-btn queue-btn-danger row-action" type="button" data-action="cancel" data-id="${escapedId}"${pendingAttrs}>${htmlEscape(strings["downloads.actionCancel"])}</button>
      </div>
    `;

    extra = `
      ${renderMetaChips(item, liveStatus)}
      <div class="queue-card-progress">
        <div class="progress-track" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${htmlEscape(Math.round(percent))}" aria-label="${htmlEscape(fileName)} progress">
          <div class="${progressFillClass}" style="width:${isIndeterminate ? "35" : percent.toFixed(1)}%"></div>
        </div>
      </div>
      <div class="queue-card-meta">${htmlEscape(summary)}</div>
    `;
  } else if (group === "failed") {
    const failure = summarizeFailure(item);

    actions = `
      <div class="queue-card-actions">
        ${liveStatus === "failed"
          ? `<button class="queue-btn queue-btn-primary row-action" type="button" data-action="retry" data-id="${escapedId}"${pendingAttrs}>Retry</button>`
          : ""}
        <button class="queue-btn queue-btn-secondary row-action" type="button" data-action="remove" data-id="${escapedId}"${pendingAttrs}>${htmlEscape(strings["actions.remove"])}</button>
      </div>
    `;

    extra = `
      ${renderMetaChips(item, liveStatus)}
      <div class="queue-failure-panel">
        <div class="queue-failure-title">${htmlEscape(failure.title)}</div>
        <div class="queue-failure-copy">${htmlEscape(failure.body)}</div>
        ${failure.detail ? `<details class="queue-failure-detail"><summary>View full error log</summary><div>${htmlEscape(failure.detail)}</div></details>` : ""}
      </div>
    `;
  } else {
    const sizeText = item.totalBytes ? formatBytes(item.totalBytes) : "Unknown size";
    actions = `
      <div class="queue-card-actions">
        <button class="queue-btn queue-btn-secondary row-action" type="button" data-action="open" data-id="${escapedId}"${pendingAttrs}>${htmlEscape(strings["downloads.actionOpen"])}</button>
        <button class="queue-btn queue-btn-secondary row-action" type="button" data-action="remove" data-id="${escapedId}"${pendingAttrs}>${htmlEscape(strings["actions.remove"])}</button>
      </div>
    `;

    extra = `
      ${renderMetaChips(item, liveStatus)}
      <div class="queue-card-meta">${htmlEscape(sizeText)} • Completed</div>
    `;
  }

  return `
    <article class="download-item queue-card queue-card--${htmlEscape(group)}" tabindex="0" data-row-id="${escapedId}" data-row-index="${index}" data-status="${htmlEscape(liveStatus)}" aria-label="${escapedName}">
      <div class="queue-card-rail"></div>
      <div class="file-ext file-ext--${htmlEscape(category)}" aria-hidden="true">
        ${htmlEscape(ext.toUpperCase())}
        <div class="file-ext-dot"></div>
      </div>
      <div class="queue-card-body">
        <div class="queue-card-head">
          <div class="queue-card-heading">
            <span class="queue-card-title">${escapedName}</span>
            <span class="queue-card-url" title="${escapedUrl}">${escapedUrl}</span>
          </div>
          <span class="pill pill--${htmlEscape(liveStatus)}">${htmlEscape(pillLabel)}</span>
        </div>
        ${extra}
        ${actions}
      </div>
    </article>
  `;
}

function renderQueue(items) {
  if (renderQueueFrame) {
    cancelAnimationFrame(renderQueueFrame);
  }
  renderQueueFrame = requestAnimationFrame(() => {
    renderQueueSync(items);
  });
}

function renderQueueSync(items) {
  const list = document.getElementById("downloadsList");
  const empty = document.getElementById("emptyState");
  const strings = window.__KHUKRI_STRINGS__;
  const hydratedItems = mergeQueueWithProgress(items);
  const focusedRowId = document.activeElement?.closest(".download-item")?.dataset?.rowId;

  if (!hydratedItems.length) {
    empty.hidden = false;
    list.hidden = true;
    list.innerHTML = "";
    return;
  }

  empty.hidden = true;
  list.hidden = false;

  const groups = {
    active: hydratedItems.filter((item) => queueGroupForStatus(safeStatus(item.liveStatus ?? item.status)) === "active"),
    failed: hydratedItems.filter((item) => queueGroupForStatus(safeStatus(item.liveStatus ?? item.status)) === "failed"),
    complete: hydratedItems.filter((item) => queueGroupForStatus(safeStatus(item.liveStatus ?? item.status)) === "complete")
  };

  let rowIndex = 0;
  list.innerHTML = ["active", "failed", "complete"]
    .filter((group) => groups[group].length > 0)
    .map((group) => `
      <section class="queue-section queue-section--${group}">
        <header class="queue-section-head">
          <span class="queue-section-marker" aria-hidden="true"></span>
          <h3 class="queue-section-title">${sectionHeading(group)} <span>(${groups[group].length})</span></h3>
        </header>
        <div class="queue-section-list">
          ${groups[group].map((item) => renderDownloadCard(item, rowIndex++, strings)).join("")}
        </div>
      </section>
    `)
    .join("");

  if (focusedRowId) {
    list.querySelector(`[data-row-id="${focusedRowId}"]`)?.focus();
  }
}

function showView(view) {
  currentView = view;
  document.querySelectorAll("[data-view]").forEach((section) => {
    section.hidden = section.dataset.view !== view;
  });

  if (view !== "downloads") {
    const composer = document.getElementById("downloadsComposer");
    if (composer) {
      composer.hidden = true;
    }
  }

  document.getElementById("navDownloads").classList.toggle("nav-item-active", view === "downloads");
  document.getElementById("navSettings").classList.toggle("nav-item-active", view === "settings");
}

function resolvedTheme(themeMode) {
  if (themeMode === "dark" || themeMode === "light") {
    return themeMode;
  }

  return window.matchMedia?.("(prefers-color-scheme: dark)")?.matches ? "dark" : "light";
}

function applyTheme(settings) {
  const themeMode = settings?.appearance?.theme || "system";
  const nextTheme = resolvedTheme(themeMode);
  document.documentElement.dataset.theme = nextTheme;
  persistThemePreference(themeMode);
}

function syncDownloadDefaults(settings) {
  const outputPath = document.getElementById("outputPath");
  outputPath.placeholder = settings.general.defaultDownloadPath || outputPath.placeholder;

  const throttle = document.getElementById("throttle");
  if (!throttle.value && settings.performance.bandwidthCap) {
    throttle.value = String(settings.performance.bandwidthCap);
  }
}

function populateSettingsForm(settings) {
  document.getElementById("settingsDefaultPath").value = settings.general.defaultDownloadPath || "";
  document.getElementById("settingsMaxConcurrent").value = String(settings.general.maxConcurrent ?? 3);
  document.getElementById("settingsYtdlpAutoUpdate").checked = settings.ytdlp_auto_update !== false;
  document.getElementById("settingsThreadOverride").value = settings.performance.threadOverride ?? "";
  document.getElementById("settingsBandwidthCap").value = settings.performance.bandwidthCap ?? "";
  document.getElementById("settingsSchedulerEnabled").checked = Boolean(settings.scheduler.enabled);
  document.getElementById("settingsSchedulerStart").value = String(settings.scheduler.startHour ?? 0);
  document.getElementById("settingsSchedulerEnd").value = String(settings.scheduler.endHour ?? 23);
  document.getElementById("settingsProxyEnabled").checked = Boolean(settings.proxy.enabled);
  document.getElementById("settingsProxyUrl").value = settings.proxy.url || "";
  document.getElementById("settingsTheme").value = settings.appearance.theme || "system";
}

function canonicalSettings(settings) {
  return JSON.stringify(settings);
}

function settingsMatchForm() {
  if (!currentSettings) {
    return true;
  }
  return canonicalSettings(collectSettingsForm()) === canonicalSettings(currentSettings);
}

function toggleSettingsDependencies() {
  const schedulerEnabled = document.getElementById("settingsSchedulerEnabled").checked;
  const proxyEnabled = document.getElementById("settingsProxyEnabled").checked;
  document.getElementById("settingsSchedulerStart").disabled = !schedulerEnabled;
  document.getElementById("settingsSchedulerEnd").disabled = !schedulerEnabled;
  document.getElementById("settingsProxyUrl").disabled = !proxyEnabled;
}

function renderSettingsStatus() {
  const statusNode = document.getElementById("settingsStatus");
  const saveButton = document.getElementById("saveSettingsButton");
  settingsDirty = !settingsMatchForm();
  saveButton.disabled = !settingsDirty;
  saveButton.setAttribute("aria-disabled", String(!settingsDirty));
  document.body.classList.toggle("settings-dirty", settingsDirty);

  if (!settingsDirty && !statusNode.dataset.state) {
    statusNode.textContent = "";
    return;
  }

  if (settingsDirty) {
    statusNode.dataset.state = "dirty";
    statusNode.innerHTML = `<span class="settings-status-dot" aria-hidden="true"></span>${htmlEscape(window.__KHUKRI_STRINGS__["settings.unsaved"])}`;
    return;
  }

  if (statusNode.dataset.state === "dirty") {
    statusNode.textContent = "";
    delete statusNode.dataset.state;
  }
}

function collectSettingsForm() {
  const readInt = (id) => {
    const value = document.getElementById(id).value.trim();
    return value ? Number(value) : null;
  };

  return {
    general: {
      defaultDownloadPath: document.getElementById("settingsDefaultPath").value.trim(),
      maxConcurrent: Math.max(1, Number(document.getElementById("settingsMaxConcurrent").value || 1))
    },
    performance: {
      threadOverride: readInt("settingsThreadOverride"),
      bandwidthCap: readInt("settingsBandwidthCap")
    },
    scheduler: {
      enabled: document.getElementById("settingsSchedulerEnabled").checked,
      startHour: Math.min(23, Math.max(0, Number(document.getElementById("settingsSchedulerStart").value || 0))),
      endHour: Math.min(23, Math.max(0, Number(document.getElementById("settingsSchedulerEnd").value || 23)))
    },
    proxy: {
      enabled: document.getElementById("settingsProxyEnabled").checked,
      url: document.getElementById("settingsProxyUrl").value.trim()
    },
    appearance: {
      theme: document.getElementById("settingsTheme").value
    },
    onboarding_complete: onboardingComplete(currentSettings),
    ytdlp_auto_update: document.getElementById("settingsYtdlpAutoUpdate").checked,
    ytdlp_last_check: currentSettings?.ytdlp_last_check ?? null,
    ytdlp_version: currentSettings?.ytdlp_version ?? null,
    ytdlp_last_notified_failure: currentSettings?.ytdlp_last_notified_failure ?? null,
    ytdlp_last_rate_limit: Boolean(currentSettings?.ytdlp_last_rate_limit)
  };
}

function applySettings(settings) {
  currentSettings = settings;
  populateSettingsForm(settings);
  syncDownloadDefaults(settings);
  applyTheme(settings);
  toggleOnboarding(settings);
  toggleSettingsDependencies();
  renderSettingsStatus();
}

async function refreshQueue(strings, options = {}) {
  if (queueRefreshInFlight) {
    return;
  }

  queueRefreshInFlight = true;
  const statusNode = document.getElementById("formStatus");
  if (!options.preserveStatus) {
    statusNode.textContent = strings["status.loading"];
  }
  try {
    const queue = await invoke("get_queue");
    currentQueue = queue;
    renderQueue(queue);
    if (!options.preserveStatus) {
      setStatusReady(statusNode, strings["status.ready"]);
    }
  } catch (error) {
    statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
  } finally {
    queueRefreshInFlight = false;
  }
}

async function handleRowAction(action, id) {
  const strings = window.__KHUKRI_STRINGS__;
  const mergedQueue = mergeQueueWithProgress(currentQueue);
  const item = mergedQueue.find((entry) => entry.id === id);
  if (!item) {
    return;
  }
  if (pendingActions.has(id)) {
    return;
  }

  const previousQueue = currentQueue.map((entry) => ({ ...entry }));
  const previousProgress = progressById.has(id)
    ? { ...progressById.get(id) }
    : undefined;

  pendingActions.add(id);
  try {
    if (action === "pause") {
      updateLocalDownloadState(id, (entry) => ({ ...entry, status: "paused" }));
      setOptimisticProgress(id, "paused");
      renderQueue(currentQueue);
      await invoke("pause_download", { id });
    } else if (action === "resume") {
      progressById.delete(id);
      updateLocalDownloadState(id, (entry) => ({ ...entry, status: "queued" }));
      renderQueue(currentQueue);
      await invoke("resume_download", { id });
    } else if (action === "cancel") {
      updateLocalDownloadState(id, (entry) => ({ ...entry, status: "cancelled" }));
      renderQueue(currentQueue);
      await invoke("cancel_download", { id });
      progressById.delete(id);
    } else if (action === "remove") {
      currentQueue = currentQueue.filter((entry) => entry.id !== id);
      renderQueue(currentQueue);
      await invoke("remove_download", { id });
      progressById.delete(id);
    } else if (action === "retry") {
      progressById.delete(id);
      updateLocalDownloadState(id, (entry) => ({ ...entry, status: "queued", failureReason: null }));
      renderQueue(currentQueue);
      await invoke("start_download", { request: buildRetryRequest(item) });
    } else if (action === "open") {
      await invoke("open_download_folder", { filePath: item.filePath });
    }
  } catch (error) {
    currentQueue = previousQueue;
    if (previousProgress) {
      progressById.set(id, previousProgress);
    } else {
      progressById.delete(id);
    }
    renderQueue(currentQueue);
    if (action === "open") {
      await refreshQueue(strings, { preserveStatus: true });
    }
    throw error;
  } finally {
    pendingActions.delete(id);
    renderQueue(currentQueue);
  }

  setStatusReady(document.getElementById("formStatus"), strings["status.ready"]);
  await refreshQueue(strings);
}

async function saveSettings() {
  const strings = window.__KHUKRI_STRINGS__;
  const statusNode = document.getElementById("settingsStatus");
  statusNode.dataset.state = "loading";
  statusNode.textContent = strings["status.loading"];
  const settings = collectSettingsForm();
  const nextSettings = await invoke("update_settings", { settings });
  currentSettings = nextSettings;
  applySettings(nextSettings);
  statusNode.dataset.state = "saved";
  statusNode.innerHTML = `<span class="settings-saved-indicator">${htmlEscape(strings["settings.saved"])}</span>`;
}

async function resetSettingsSection(section) {
  currentSettings = await invoke("reset_settings_section", { section });
  applySettings(currentSettings);
}

async function acknowledgeOnboarding() {
  const nextSettings = await invoke("acknowledge_media_onboarding");
  applySettings(nextSettings);
}

async function checkYtdlpNow() {
  await invoke("check_ytdlp_updates_now");
}

async function registerCloseToTray() {
  const currentWindow = getCurrentWindowHandle();
  if (!currentWindow?.onCloseRequested) {
    return;
  }

  await currentWindow.onCloseRequested(async (event) => {
    event.preventDefault();
    await currentWindow.hide();
  });
}

function registerThemeWatcher() {
  const media = window.matchMedia?.("(prefers-color-scheme: dark)");
  if (!media?.addEventListener) {
    return;
  }

  media.addEventListener("change", () => {
    if (currentSettings?.appearance?.theme === "system") {
      applyTheme(currentSettings);
    }
  });
}

async function main() {
  const strings = await loadStrings();
  window.__KHUKRI_STRINGS__ = strings;
  applyStrings(strings);

  const form = document.getElementById("downloadForm");
  const settingsForm = document.getElementById("settingsForm");
  const refreshButton = document.getElementById("refreshQueue");
  const newDownloadButton = document.getElementById("newDownload");
  const statusNode = document.getElementById("formStatus");
  const settingsStatus = document.getElementById("settingsStatus");
  const downloadsList = document.getElementById("downloadsList");
  const onboardingButton = document.getElementById("acknowledgeOnboarding");
  const checkYtdlpButton = document.getElementById("checkYtdlpNow");
  const saveSettingsButton = document.getElementById("saveSettingsButton");

  showView("downloads");
  registerThemeWatcher();
  await registerCloseToTray();

  currentSettings = await invoke("get_settings");
  applySettings(currentSettings);
  document.body.classList.add("app-ready");

  const composer = document.getElementById("downloadsComposer");

  newDownloadButton.addEventListener("click", () => {
    composer.hidden = !composer.hidden;
    if (!composer.hidden) {
      document.getElementById("downloadUrl").focus();
    }
  });

  document.getElementById("composerClose").addEventListener("click", () => {
    composer.hidden = true;
  });

  onboardingButton?.addEventListener("click", () => {
    settingsStatus.dataset.state = "loading";
    settingsStatus.textContent = strings["status.loading"];
    void acknowledgeOnboarding().then(() => {
      settingsStatus.dataset.state = "saved";
      settingsStatus.innerHTML = `<span class="settings-saved-indicator">${htmlEscape(strings["settings.saved"])}</span>`;
    }).catch((error) => {
      settingsStatus.dataset.state = "failed";
      settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    });
  });

  checkYtdlpButton?.addEventListener("click", () => {
    settingsStatus.dataset.state = "loading";
    settingsStatus.textContent = strings["ytdlp.updateStarted"];
    void checkYtdlpNow().catch((error) => {
      settingsStatus.dataset.state = "failed";
      settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    });
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const formData = new FormData(form);
    const throttle = Number(formData.get("throttle") || 0);
    const request = {
      url: String(formData.get("downloadUrl") || ""),
      filePath: String(formData.get("outputPath") || ""),
      priority: String(formData.get("priority") || "normal"),
      bytesPerSec: throttle > 0 ? throttle : null,
      quality: String(formData.get("quality") || "") || null
    };

    try {
      statusNode.textContent = strings["status.loading"];
      await invoke("start_download", { request });
      form.reset();
      syncDownloadDefaults(currentSettings);
      composer.hidden = true;
      await refreshQueue(strings, { preserveStatus: true });
      statusNode.textContent = strings["status.started"];
    } catch (error) {
      statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    }
  });

  document.getElementById("browseDefaultPath").addEventListener("click", () => {
    void invoke("pick_folder").then((picked) => {
      if (picked) {
        document.getElementById("settingsDefaultPath").value = picked;
        renderSettingsStatus();
      }
    }).catch((error) => {
      settingsStatus.dataset.state = "failed";
      settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    });
  });

  settingsForm.addEventListener("submit", (event) => {
    event.preventDefault();
    if (!settingsDirty) {
      return;
    }
    void saveSettings().catch((error) => {
      settingsStatus.dataset.state = "failed";
      settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    });
  });

  settingsForm.addEventListener("input", () => {
    toggleSettingsDependencies();
    renderSettingsStatus();
  });

  settingsForm.addEventListener("change", () => {
    toggleSettingsDependencies();
    renderSettingsStatus();
  });

  document.querySelectorAll(".reset-section").forEach((button) => {
    button.addEventListener("click", () => {
      const sectionName = button.closest(".settings-section")?.querySelector(".section-label")?.textContent || "this section";
      if (!window.confirm(`Reset ${sectionName} to defaults?`)) {
        return;
      }
      settingsStatus.dataset.state = "loading";
      settingsStatus.textContent = strings["status.loading"];
      void resetSettingsSection(button.dataset.section).then(() => {
        settingsStatus.dataset.state = "saved";
        settingsStatus.innerHTML = `<span class="settings-saved-indicator">${htmlEscape(strings["settings.saved"])}</span>`;
      }).catch((error) => {
        settingsStatus.dataset.state = "failed";
        settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
      });
    });
  });

  window.addEventListener("beforeunload", (event) => {
    if (!settingsDirty) {
      return;
    }
    event.preventDefault();
    event.returnValue = "";
  });

  refreshButton.addEventListener("click", () => {
    void refreshQueue(strings);
  });

  document.getElementById("navDownloads").addEventListener("click", () => {
    showView("downloads");
  });

  document.getElementById("navSettings").addEventListener("click", () => {
    showView("settings");
  });

  downloadsList.addEventListener("click", (event) => {
    const button = event.target.closest(".row-action");
    if (!button) {
      return;
    }

    statusNode.textContent = strings["status.loading"];
    void handleRowAction(button.dataset.action, button.dataset.id).catch((error) => {
      statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    });
  });

  downloadsList.addEventListener("keydown", (event) => {
    const row = event.target.closest(".download-item");
    if (!row) {
      return;
    }

    const rows = Array.from(downloadsList.querySelectorAll(".download-item"));
    const index = Number(row.dataset.rowIndex || "0");
    if (event.key === "ArrowDown") {
      event.preventDefault();
      rows[Math.min(index + 1, rows.length - 1)]?.focus();
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      rows[Math.max(index - 1, 0)]?.focus();
      return;
    }

    if (event.key === "Delete") {
      event.preventDefault();
      const item = mergeQueueWithProgress(currentQueue).find((entry) => entry.id === row.dataset.rowId);
      if (!item) {
        return;
      }
      const liveStatus = item.liveStatus ?? item.status;
      const action = ["active", "queued", "paused"].includes(liveStatus) ? "cancel" : "remove";
      statusNode.textContent = strings["status.loading"];
      void handleRowAction(action, row.dataset.rowId).catch((error) => {
        statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
      });
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      statusNode.textContent = strings["status.loading"];
      const item = mergeQueueWithProgress(currentQueue).find((entry) => entry.id === row.dataset.rowId);
      if (!item) {
        return;
      }
      void handleRowAction(rowPrimaryAction(item), row.dataset.rowId).catch((error) => {
        statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
      });
    }
  });

  const eventApi = window.__TAURI__?.event;
  if (eventApi?.listen) {
    await eventApi.listen("download-progress", (event) => {
      if (event.payload?.id) {
        const previous = progressById.get(event.payload.id);
        const isTerminal = ["paused", "failed", "complete", "cancelled"].includes(event.payload.status);
        const resumesFromTerminal = previous
          && ["paused", "failed", "complete", "cancelled"].includes(previous.status)
          && ["queued", "active"].includes(event.payload.status);
        if (!previous || event.payload.bytesDone >= previous.bytesDone || isTerminal || resumesFromTerminal) {
          progressById.set(event.payload.id, event.payload);
        }
        if (["failed", "complete", "cancelled"].includes(event.payload.status)) {
          progressById.delete(event.payload.id);
        }
      }
      renderQueue(currentQueue);
    });

    await eventApi.listen("queue-updated", (event) => {
      if (Array.isArray(event.payload)) {
        currentQueue = event.payload;
        const liveIds = new Set(currentQueue.map((item) => item.id));
        currentQueue.forEach((item) => {
          const progress = progressById.get(item.id);
          const keepLiveProgress = item.status === "paused"
            && Boolean(progress)
            && progress.status === "paused";
          if (!keepLiveProgress && ["paused", "failed", "complete", "cancelled"].includes(item.status)) {
            progressById.delete(item.id);
          }
        });
        Array.from(progressById.keys()).forEach((id) => {
          if (!liveIds.has(id)) {
            progressById.delete(id);
          }
        });
      }
      renderQueue(currentQueue);
      void invoke("pump_queue").catch(() => { });
    });

    await eventApi.listen("settings-updated", (event) => {
      if (event.payload) {
        applySettings(event.payload);
      }
    });

    await eventApi.listen("ytdlp-update-status", (event) => {
      if (!event.payload) {
        return;
      }
      if (event.payload.kind === "updated") {
        settingsStatus.dataset.state = "saved";
        settingsStatus.textContent = event.payload.message || strings["ytdlp.updateApplied"];
        void invoke("get_settings").then((settings) => {
          applySettings(settings);
        }).catch(() => {});
        return;
      }
      if (event.payload.kind === "noop") {
        settingsStatus.dataset.state = "saved";
        settingsStatus.textContent = event.payload.message || strings["ytdlp.updateNoop"];
        void invoke("get_settings").then((settings) => {
          applySettings(settings);
        }).catch(() => {});
        return;
      }
      if (event.payload.kind === "failed") {
        settingsStatus.dataset.state = "failed";
        settingsStatus.textContent = event.payload.message || strings["status.failed"];
      }
    });
  }

  await refreshQueue(strings);
  void invoke("pump_queue").catch(() => { });

  // Keep queue view feeling instant even if event delivery is delayed.
  window.setInterval(() => {
    void refreshQueue(strings, { preserveStatus: true });
  }, 1200);
}

function showBootError(error) {
  console.error("Khukri failed to start", error);
  const splash = document.getElementById("bootSplash");
  if (!splash) {
    return;
  }
  splash.innerHTML = `
    <div class="boot-error">
      <strong>Khukri failed to start.</strong>
      <span>${htmlEscape(errorText(error))}</span>
    </div>
  `;
}

void main().catch(showBootError);
