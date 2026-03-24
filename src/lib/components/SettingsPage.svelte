<script lang="ts">
  import { onMount } from "svelte";
  import Slider from "./Slider.svelte";
  import { t, locale, localeOptions } from "$lib/i18n";
  import type { Locale } from "$lib/i18n";
  import { isLoggedIn, authLoading, authMessage } from "$lib/stores/auth";
  import { viewMode, cardSize, detailPreviewSize, theme } from "$lib/stores/ui";
  import type { Theme } from "$lib/stores/ui";
  import { login, logout } from "$lib/api/auth";
  import { getDetailPreviewSize, setDetailPreviewSize, getCacheDir, setCacheDir, clearImageCache, setTheme } from "$lib/api/reader";

  // Account
  let ipbMemberId = $state("");
  let ipbPassHash = $state("");
  let igneous = $state("");

  // Theme — wired to store + IPC
  function handleThemeChange(newTheme: Theme) {
    $theme = newTheme;
    setTheme(newTheme).catch(() => {});
  }

  // Network placeholders
  let proxyType = $state<"none" | "http" | "socks5">("none");
  let proxyHost = $state("");
  let proxyPort = $state("");

  // Download placeholders
  let downloadDir = $state("");
  let downloadQuality = $state<"original" | "resampled">("original");
  let resumeDownloads = $state(true);

  let activeSection = $state("account");

  // Cache management state
  let cacheDir = $state("");
  let cacheDirLoading = $state(false);
  let clearingCache = $state(false);
  let cacheMessage = $state("");

  onMount(async () => {
    try {
      cacheDir = await getCacheDir();
    } catch {}
  });

  const sections = [
    { id: "account", key: "settings.account" },
    { id: "theme", key: "settings.theme" },
    { id: "preference", key: "settings.preference" },
    { id: "storage", key: "settings.storage" },
    { id: "network", key: "settings.network" },
    { id: "downloads", key: "settings.downloads" },
    { id: "about", key: "settings.about" },
  ];

  async function handleLogin(e: Event) {
    e.preventDefault();
    if (!ipbMemberId.trim() || !ipbPassHash.trim() || !igneous.trim()) {
      $authMessage = $t("auth.fields_required");
      return;
    }
    $authLoading = true;
    $authMessage = "";
    try {
      const result = await login(ipbMemberId.trim(), ipbPassHash.trim(), igneous.trim());
      $authMessage = result.message;
      if (result.success) {
        $isLoggedIn = true;
        ipbMemberId = "";
        ipbPassHash = "";
        igneous = "";
      }
    } catch (err) {
      $authMessage = `Error: ${err}`;
    } finally {
      $authLoading = false;
    }
  }

  async function handleLogout() {
    $authLoading = true;
    try {
      const result = await logout();
      $authMessage = result.message;
      $isLoggedIn = false;
    } catch (err) {
      $authMessage = `Error: ${err}`;
    } finally {
      $authLoading = false;
    }
  }

  function handleLocaleChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value as Locale;
    $locale = val;
  }

  async function handleClearCache() {
    if (!confirm($t("settings.clear_cache_confirm"))) return;
    clearingCache = true;
    cacheMessage = "";
    try {
      const bytesFreed = await clearImageCache();
      const sizeMb = (bytesFreed / 1024 / 1024).toFixed(1);
      const sizeStr = bytesFreed > 1024 * 1024 ? `${sizeMb} MB` : `${Math.round(bytesFreed / 1024)} KB`;
      cacheMessage = $t("settings.cache_cleared", { size: sizeStr });
    } catch (err) {
      cacheMessage = `Error: ${err}`;
    } finally {
      clearingCache = false;
    }
  }

  async function handleResetCacheDir() {
    cacheDirLoading = true;
    try {
      await setCacheDir("");
      cacheDir = await getCacheDir();
    } catch (err) {
      cacheMessage = `Error: ${err}`;
    } finally {
      cacheDirLoading = false;
    }
  }

  async function handleSaveCacheDir() {
    cacheDirLoading = true;
    try {
      await setCacheDir(cacheDir);
    } catch (err) {
      cacheMessage = `Error: ${err}`;
    } finally {
      cacheDirLoading = false;
    }
  }
</script>

<div class="settings-page">
  <div class="settings-nav">
    <h2>{$t("settings.title")}</h2>
    {#each sections as section}
      <button
        class="settings-nav-item"
        class:active={activeSection === section.id}
        onclick={() => activeSection = section.id}
      >
        {$t(section.key)}
      </button>
    {/each}
  </div>

  <div class="settings-content">
    {#if activeSection === "account"}
      <div class="section">
        <h3>{$t("settings.account")}</h3>
        {#if $isLoggedIn}
          <div class="account-status">
            <span class="connected-dot"></span>
            <span>{$t("auth.connected")}</span>
            <button class="btn-danger" onclick={handleLogout} disabled={$authLoading}>
              {$t("auth.logout")}
            </button>
          </div>
        {/if}

        <div class="subsection">
          <h4>{$t("auth.manual_cookie")}</h4>
          <form class="cookie-form" onsubmit={handleLogin}>
            <label class="field">
              <span>ipb_member_id</span>
              <input type="text" bind:value={ipbMemberId} placeholder="e.g. 1234567" disabled={$authLoading} />
            </label>
            <label class="field">
              <span>ipb_pass_hash</span>
              <input type="password" bind:value={ipbPassHash} placeholder="e.g. abc123def456..." disabled={$authLoading} />
            </label>
            <label class="field">
              <span>igneous</span>
              <input type="password" bind:value={igneous} placeholder="e.g. a1b2c3d4..." disabled={$authLoading} />
            </label>
            <button type="submit" class="btn-primary" disabled={$authLoading}>
              {$authLoading ? $t("auth.validating") : $t("auth.login")}
            </button>
          </form>
          {#if $authMessage}
            <p class="message" class:error={!$isLoggedIn} class:success={$isLoggedIn}>{$authMessage}</p>
          {/if}
        </div>

        <div class="subsection">
          <button class="btn-outline" disabled>
            {$t("auth.login_browser")}
          </button>
          <p class="hint">{$t("settings.coming_soon")}</p>
        </div>
      </div>

    {:else if activeSection === "theme"}
      <div class="section">
        <h3>{$t("settings.theme")}</h3>

        <div class="field-row">
          <label class="field-label">{$t("settings.color_scheme")}</label>
          <div class="btn-group">
            <button class:active={$theme === "light"} onclick={() => handleThemeChange("light")}>{$t("settings.light")}</button>
            <button class:active={$theme === "dark"} onclick={() => handleThemeChange("dark")}>{$t("settings.dark")}</button>
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.view_mode")}</label>
          <div class="btn-group">
            <button class:active={$viewMode === "cards"} onclick={() => $viewMode = "cards"}>{$t("settings.cards")}</button>
            <button class:active={$viewMode === "list"} onclick={() => $viewMode = "list"}>{$t("settings.list")}</button>
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.card_size")}</label>
          <div class="slider-row">
            <Slider min={120} max={280} bind:value={$cardSize} />
            <span class="slider-value">{$cardSize}px</span>
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.detail_preview_size")}</label>
          <div class="slider-row">
            <Slider min={80} max={200} bind:value={$detailPreviewSize} onChange={(v) => { $detailPreviewSize = v; setDetailPreviewSize(v).catch(() => {}); }} />
            <span class="slider-value">{$detailPreviewSize}px</span>
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.layout_preset")}</label>
          <div class="btn-group">
            <button class:active={true}>{$t("settings.desktop")}</button>
            <button disabled>{$t("settings.tablet")}</button>
            <button disabled>{$t("settings.phone")}</button>
          </div>
          <p class="hint">{$t("settings.coming_soon")}</p>
        </div>
      </div>

    {:else if activeSection === "preference"}
      <div class="section">
        <h3>{$t("settings.preference")}</h3>

        <div class="field-row">
          <label class="field-label">{$t("settings.program_language")}</label>
          <select onchange={handleLocaleChange} value={$locale}>
            {#each localeOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
      </div>

    {:else if activeSection === "storage"}
      <div class="section">
        <h3>{$t("settings.storage")}</h3>

        <div class="field-row">
          <label class="field-label">{$t("settings.cache_folder")}</label>
          <div class="dir-row">
            <input type="text" bind:value={cacheDir} disabled={cacheDirLoading} />
            <button class="btn-outline" onclick={handleSaveCacheDir} disabled={cacheDirLoading}>
              {$t("common.confirm")}
            </button>
            <button class="btn-outline" onclick={handleResetCacheDir} disabled={cacheDirLoading}>
              {$t("settings.reset_default")}
            </button>
          </div>
          <p class="hint">{$t("settings.cache_folder_hint")}</p>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.clear_cache")}</label>
          <div>
            <button class="btn-danger" onclick={handleClearCache} disabled={clearingCache}>
              {clearingCache ? $t("settings.clearing_cache") : $t("settings.clear_cache")}
            </button>
          </div>
          <p class="hint">{$t("settings.clear_cache_hint")}</p>
          {#if cacheMessage}
            <p class="message success">{cacheMessage}</p>
          {/if}
        </div>
      </div>

    {:else if activeSection === "network"}
      <div class="section">
        <h3>{$t("settings.network")}</h3>

        <div class="field-row">
          <label class="field-label">{$t("settings.proxy_type")}</label>
          <select bind:value={proxyType} disabled>
            <option value="none">None</option>
            <option value="http">HTTP</option>
            <option value="socks5">SOCKS5</option>
          </select>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.proxy_host")}</label>
          <input type="text" bind:value={proxyHost} placeholder="127.0.0.1" disabled />
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.proxy_port")}</label>
          <input type="text" bind:value={proxyPort} placeholder="1080" disabled />
        </div>

        <p class="hint">{$t("settings.coming_soon")}</p>
      </div>

    {:else if activeSection === "downloads"}
      <div class="section">
        <h3>{$t("settings.downloads")}</h3>

        <div class="field-row">
          <label class="field-label">{$t("settings.download_dir")}</label>
          <div class="dir-row">
            <input type="text" bind:value={downloadDir} placeholder="Select directory..." disabled />
            <button class="btn-outline" disabled>{$t("settings.load_existing")}</button>
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.download_quality")}</label>
          <div class="btn-group">
            <button class:active={downloadQuality === "original"} onclick={() => downloadQuality = "original"} disabled>{$t("settings.original")}</button>
            <button class:active={downloadQuality === "resampled"} onclick={() => downloadQuality = "resampled"} disabled>{$t("settings.resampled")}</button>
          </div>
        </div>

        <div class="field-row">
          <label class="field-label">{$t("settings.resume_downloads")}</label>
          <label class="toggle">
            <input type="checkbox" bind:checked={resumeDownloads} disabled />
            <span class="toggle-slider"></span>
          </label>
        </div>

        <p class="hint">{$t("settings.coming_soon")}</p>
      </div>

    {:else if activeSection === "about"}
      <div class="section">
        <h3>{$t("settings.about")}</h3>
        <div class="about-info">
          <p><strong>{$t("app.name")}</strong></p>
          <p>{$t("settings.app_version")}: 0.1.0</p>
          <p class="hint">Tauri 2 + Rust + Svelte + SQLite</p>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .settings-page {
    display: flex;
    height: 100%;
    overflow: hidden;
  }

  .settings-nav {
    width: 200px;
    flex-shrink: 0;
    padding: 1.25rem 0.75rem;
    border-right: 1px solid var(--border);
    background: var(--bg-primary);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .settings-nav h2 {
    margin: 0 0 0.75rem 0.5rem;
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .settings-nav-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.82rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .settings-nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .settings-nav-item.active {
    background: var(--accent-subtle);
    color: var(--accent);
    font-weight: 600;
  }

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem 2.5rem;
  }

  .section {
    max-width: 560px;
  }

  .section h3 {
    margin: 0 0 1rem;
    font-size: 1rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .subsection {
    margin-bottom: 1.25rem;
  }

  .subsection h4 {
    margin: 0 0 0.6rem;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .cookie-form {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .field span {
    font-size: 0.72rem;
    font-weight: 600;
    font-family: 'SF Mono', 'Fira Code', monospace;
    color: var(--text-secondary);
  }

  .field-row {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 1rem;
  }

  .field-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-secondary);
  }

  input[type="text"],
  input[type="password"],
  select {
    padding: 0.55rem 0.8rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
    max-width: 320px;
  }

  input:focus,
  select:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  input:disabled,
  select:disabled {
    opacity: 0.4;
  }

  .btn-primary {
    padding: 0.5rem 1.2rem;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    align-self: flex-start;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-outline {
    padding: 0.45rem 0.85rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-outline:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .btn-outline:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .btn-danger {
    padding: 0.35rem 0.8rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--danger-border);
    background: transparent;
    color: var(--red);
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-danger:hover:not(:disabled) {
    background: var(--danger-bg);
  }

  .btn-danger:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn-group {
    display: inline-flex;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .btn-group button {
    padding: 0.35rem 0.75rem;
    border: none;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-group button:last-child {
    border-right: none;
  }

  .btn-group button:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .btn-group button.active {
    background: var(--accent-subtle);
    color: var(--accent);
    font-weight: 600;
  }

  .btn-group button:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    max-width: 320px;
  }

  .slider-value {
    font-size: 0.72rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    min-width: 40px;
  }

  .dir-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .dir-row input {
    flex: 1;
  }

  .toggle {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
    cursor: pointer;
  }

  .toggle input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .toggle-slider {
    position: absolute;
    inset: 0;
    background: var(--bg-hover);
    border-radius: 10px;
    transition: background 0.2s;
  }

  .toggle-slider::before {
    content: "";
    position: absolute;
    height: 14px;
    width: 14px;
    left: 3px;
    bottom: 3px;
    background: var(--text-muted);
    border-radius: 50%;
    transition: transform 0.2s, background 0.2s;
  }

  .toggle input:checked + .toggle-slider {
    background: var(--accent);
  }

  .toggle input:checked + .toggle-slider::before {
    transform: translateX(16px);
    background: #fff;
  }

  .account-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 1rem;
    font-size: 0.82rem;
    color: var(--text-secondary);
  }

  .connected-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--green);
  }

  .message {
    padding: 0.45rem 0.7rem;
    border-radius: var(--radius-sm);
    font-size: 0.78rem;
    margin: 0.5rem 0 0;
  }

  .message.error {
    background: var(--danger-bg);
    color: var(--red);
  }

  .message.success {
    background: var(--success-bg);
    color: var(--green);
  }

  .hint {
    font-size: 0.72rem;
    color: var(--text-muted);
    margin: 0.3rem 0 0;
    opacity: 0.7;
  }

  .about-info {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .about-info p {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-secondary);
  }
</style>
