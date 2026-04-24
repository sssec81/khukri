const FALLBACK_STRINGS = {
  "app.title": "Khukri",
  "nav.downloads": "Downloads",
  "nav.settings": "Settings",
  "composer.urlLabel": "Download URL",
  "composer.outputLabel": "Output path",
  "composer.priorityLabel": "Priority",
  "composer.throttleLabel": "Throttle bytes/sec",
  "priority.high": "High",
  "priority.normal": "Normal",
  "priority.low": "Low",
  "actions.start": "Start Download",
  "actions.refresh": "Refresh",
  "actions.new_download": "New Download",
  "actions.remove": "Remove",
  "downloads.emptyTitle": "No downloads yet",
  "downloads.emptyCopy": "Click New Download to add a file to the queue.",
  "downloads.unknownBytes": "unknown",
  "downloads.actionOpen": "Open Folder",
  "downloads.actionPause": "Pause",
  "downloads.actionResume": "Resume",
  "downloads.actionCancel": "Cancel",
  "downloads.failureLabel": "Failure",
  "status.ready": "Ready",
  "status.loading": "Loading...",
  "status.started": "Download queued.",
  "status.failed": "Something went wrong. Check the Rust console for details.",
  "settings.title": "Preferences",
  "settings.reset": "Reset",
  "settings.save": "Save",
  "settings.saved": "Saved.",
  "settings.general.title": "General",
  "settings.general.pathLabel": "Default download path",
  "settings.general.concurrentLabel": "Max concurrent downloads",
  "settings.performance.title": "Performance",
  "settings.performance.threadsLabel": "Thread override",
  "settings.performance.bandwidthLabel": "Bandwidth cap (bytes/sec)",
  "settings.scheduler.title": "Scheduler",
  "settings.scheduler.enabledLabel": "Enable scheduler window",
  "settings.scheduler.startLabel": "Start hour",
  "settings.scheduler.endLabel": "End hour",
  "settings.proxy.title": "Proxy",
  "settings.proxy.enabledLabel": "Enable proxy",
  "settings.proxy.urlLabel": "Proxy URL",
  "settings.appearance.title": "Appearance",
  "settings.appearance.themeLabel": "Theme",
  "settings.appearance.themeSystem": "Follow system",
  "settings.appearance.themeDark": "Dark",
  "settings.appearance.themeLight": "Light"
};

const progressById = new Map();
let currentQueue = [];
let currentSettings = null;
let currentView = "downloads";

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

function rowPrimaryAction(item) {
  const status = item.liveStatus ?? item.status;
  if (status === "complete") {
    return "open";
  }
  if (status === "paused") {
    return "resume";
  }
  if (status === "failed" || status === "cancelled") {
    return "remove";
  }
  return "pause";
}

function mergeQueueWithProgress(items) {
  return items.map((item) => {
    const progress = progressById.get(item.id);
    const hasLiveResumeProgress = progress && ["queued", "active"].includes(progress.status);
    const backendTerminal = ["failed", "complete", "cancelled"].includes(item.status)
      || (item.status === "paused" && !hasLiveResumeProgress);
    const liveStatus = hasLiveResumeProgress ? progress.status : (backendTerminal ? item.status : (progress?.status ?? item.status));
    const totalBytes = progress?.totalBytes ?? item.totalBytes ?? null;
    const bytesDone = liveStatus === "complete" && totalBytes != null
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
      percent: liveStatus === "complete" ? 100 : percent
    };
  });
}

function renderQueue(items) {
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

  list.innerHTML = hydratedItems.map((item, index) => {
    const action = rowPrimaryAction(item);
    const canCancel = ["active", "queued", "paused"].includes(item.liveStatus);
    const hasPrimaryAction = action !== "none";

    let actionLabel;
    if (action === "open") {
      actionLabel = strings["downloads.actionOpen"];
    } else if (action === "resume") {
      actionLabel = strings["downloads.actionResume"];
    } else if (action === "remove") {
      actionLabel = strings["actions.remove"];
    } else if (action === "none") {
      actionLabel = "";
    } else {
      actionLabel = strings["downloads.actionPause"];
    }

    const liveStatus = item.liveStatus ?? item.status;
    const speedText = item.speedBps > 0 ? `${formatBytes(item.speedBps)}/s` : null;
    const etaText = item.etaSeconds > 0 ? formatEta(item.etaSeconds) : null;
    const sizeText = item.totalBytes ? formatBytes(item.totalBytes) : null;
    const failureText = item.failureReason || null;
    const percent = liveStatus === "complete" ? 100 : item.percent;

    return `
      <article class="download-item" tabindex="0" data-row-id="${item.id}" data-row-index="${index}" aria-label="${item.filePath}">
        <div class="row-top">
          <span class="row-name">${baseName(item.filePath)}</span>
          <span class="pill pill--${item.liveStatus}">${item.liveStatus}</span>
          <div class="row-actions">
            ${hasPrimaryAction
        ? `<button class="row-btn row-action" type="button" data-action="${action}" data-id="${item.id}">${actionLabel}</button>`
        : ""}
            ${canCancel ? `<button class="row-btn row-btn-danger row-action" type="button" data-action="cancel" data-id="${item.id}">${strings["downloads.actionCancel"]}</button>` : ""}
          </div>
        </div>
        <div class="row-bottom">
          <span class="row-url" title="${item.url}">${item.url}</span>
          <div class="row-progress">
            <div class="progress-track" aria-hidden="true">
              <div class="progress-fill progress-fill--${item.liveStatus}" style="width:${percent.toFixed(1)}%"></div>
            </div>
            <span class="row-pct">${percent.toFixed(0)}%</span>
          </div>
          ${speedText ? `<span class="row-stat">${speedText}</span>` : ""}
          ${etaText ? `<span class="row-stat">${etaText}</span>` : ""}
          ${sizeText ? `<span class="row-stat">${sizeText}</span>` : ""}
        </div>
        ${item.liveStatus === "failed" && failureText
        ? `<div class="row-error">${strings["downloads.failureLabel"]}: ${failureText}</div>`
        : ""}
      </article>
    `;
  }).join("");

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
  const nextTheme = resolvedTheme(settings?.appearance?.theme || "system");
  document.documentElement.dataset.theme = nextTheme;
}

function syncDownloadDefaults(settings) {
  const outputPath = document.getElementById("outputPath");
  if (!outputPath.value.trim()) {
    outputPath.value = settings.general.defaultDownloadPath || "";
  }
  outputPath.placeholder = settings.general.defaultDownloadPath || outputPath.placeholder;

  const throttle = document.getElementById("throttle");
  if (!throttle.value && settings.performance.bandwidthCap) {
    throttle.value = String(settings.performance.bandwidthCap);
  }
}

function populateSettingsForm(settings) {
  document.getElementById("settingsDefaultPath").value = settings.general.defaultDownloadPath || "";
  document.getElementById("settingsMaxConcurrent").value = String(settings.general.maxConcurrent ?? 3);
  document.getElementById("settingsThreadOverride").value = settings.performance.threadOverride ?? "";
  document.getElementById("settingsBandwidthCap").value = settings.performance.bandwidthCap ?? "";
  document.getElementById("settingsSchedulerEnabled").checked = Boolean(settings.scheduler.enabled);
  document.getElementById("settingsSchedulerStart").value = String(settings.scheduler.startHour ?? 0);
  document.getElementById("settingsSchedulerEnd").value = String(settings.scheduler.endHour ?? 23);
  document.getElementById("settingsProxyEnabled").checked = Boolean(settings.proxy.enabled);
  document.getElementById("settingsProxyUrl").value = settings.proxy.url || "";
  document.getElementById("settingsTheme").value = settings.appearance.theme || "system";
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
    }
  };
}

function applySettings(settings) {
  currentSettings = settings;
  populateSettingsForm(settings);
  syncDownloadDefaults(settings);
  applyTheme(settings);
}

async function refreshQueue(strings, options = {}) {
  const statusNode = document.getElementById("formStatus");
  if (!options.preserveStatus) {
    statusNode.textContent = strings["status.loading"];
  }
  try {
    const queue = await invoke("get_queue");
    currentQueue = queue;
    renderQueue(queue);
    if (!options.preserveStatus) {
      statusNode.textContent = strings["status.ready"];
    }
  } catch (error) {
    statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
  }
}

async function handleRowAction(action, id) {
  const strings = window.__KHUKRI_STRINGS__;
  const mergedQueue = mergeQueueWithProgress(currentQueue);
  const item = mergedQueue.find((entry) => entry.id === id);
  if (!item) {
    return;
  }

  if (action === "pause") {
    await invoke("pause_download", { id });
    currentQueue = currentQueue.map((entry) => entry.id === id
      ? { ...entry, status: "paused" }
      : entry);
    renderQueue(currentQueue);
  } else if (action === "resume") {
    await invoke("resume_download", { id });
  } else if (action === "cancel") {
    await invoke("cancel_download", { id });
    progressById.delete(id);
  } else if (action === "remove") {
    await invoke("remove_download", { id });
    progressById.delete(id);
  } else if (action === "open") {
    await invoke("open_download_folder", { filePath: item.filePath });
  }

  document.getElementById("formStatus").textContent = strings["status.ready"];
  await refreshQueue(strings);
}

async function saveSettings() {
  const strings = window.__KHUKRI_STRINGS__;
  const statusNode = document.getElementById("settingsStatus");
  statusNode.textContent = strings["status.loading"];
  const settings = collectSettingsForm();
  const nextSettings = await invoke("update_settings", { settings });
  currentSettings = nextSettings;
  statusNode.textContent = strings["settings.saved"];
}

async function resetSettingsSection(section) {
  currentSettings = await invoke("reset_settings_section", { section });
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

  showView("downloads");
  registerThemeWatcher();
  await registerCloseToTray();

  currentSettings = await invoke("get_settings");
  applySettings(currentSettings);

  newDownloadButton.addEventListener("click", () => {
    const composer = document.getElementById("downloadsComposer");
    composer.hidden = !composer.hidden;
    if (!composer.hidden) {
      document.getElementById("downloadUrl").focus();
    }
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const formData = new FormData(form);
    const throttle = Number(formData.get("throttle") || 0);
    const request = {
      url: String(formData.get("downloadUrl") || ""),
      filePath: String(formData.get("outputPath") || ""),
      priority: String(formData.get("priority") || "normal"),
      bytesPerSec: throttle > 0 ? throttle : null
    };

    try {
      statusNode.textContent = strings["status.loading"];
      await invoke("start_download", { request });
      form.reset();
      syncDownloadDefaults(currentSettings);
      document.getElementById("downloadsComposer").hidden = true;
      await refreshQueue(strings, { preserveStatus: true });
      statusNode.textContent = strings["status.started"];
    } catch (error) {
      statusNode.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    }
  });

  settingsForm.addEventListener("submit", (event) => {
    event.preventDefault();
    void saveSettings().catch((error) => {
      settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
    });
  });

  document.querySelectorAll(".reset-section").forEach((button) => {
    button.addEventListener("click", () => {
      settingsStatus.textContent = strings["status.loading"];
      void resetSettingsSection(button.dataset.section).then(() => {
        settingsStatus.textContent = strings["settings.saved"];
      }).catch((error) => {
        settingsStatus.textContent = `${strings["status.failed"]} ${errorText(error)}`;
      });
    });
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
      if (!item || !["active", "queued", "paused"].includes(item.liveStatus ?? item.status)) {
        return;
      }
      statusNode.textContent = strings["status.loading"];
      void handleRowAction("cancel", row.dataset.rowId).catch((error) => {
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
        if (!previous || event.payload.bytesDone >= previous.bytesDone || isTerminal) {
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
          const keepLiveProgress = item.status === "paused" && Boolean(progress);
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
  }

  await refreshQueue(strings);
  void invoke("pump_queue").catch(() => { });
}

void main();
