<script lang="ts">
  import { onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { t } from "$lib/i18n";
  import { login, openLoginWindow } from "$lib/api/auth";
  import { isLoggedIn, authLoading, authMessage } from "$lib/stores/auth";

  // true = webview login (default), false = manual cookie form
  let useWebview = $state(true);

  let ipbMemberId = $state("");
  let ipbPassHash = $state("");
  let igneous = $state("");

  // Listen for cookies delivered by the login WebView.
  let unlisten: (() => void) | null = null;
  listen<{ ipb_member_id: string; ipb_pass_hash: string; igneous: string }>(
    "webview-login-cookies",
    async (event) => {
      const { ipb_member_id, ipb_pass_hash, igneous: ign } = event.payload;
      $authLoading = true;
      $authMessage = "";
      try {
        const result = await login(ipb_member_id, ipb_pass_hash, ign);
        $authMessage = result.message;
        if (result.success) $isLoggedIn = true;
      } catch (err) {
        $authMessage = `Error: ${err}`;
      } finally {
        $authLoading = false;
      }
    }
  ).then((fn) => { unlisten = fn; });

  onDestroy(() => unlisten?.());

  function switchMode() {
    $authMessage = "";
    useWebview = !useWebview;
  }

  async function handleOpenWindow() {
    $authMessage = "";
    $authLoading = true;
    try {
      await openLoginWindow();
    } catch (err) {
      $authMessage = `Error: ${err}`;
    }
    // authLoading cleared when webview-login-cookies fires (or user closes window)
    $authLoading = false;
  }

  async function handleCookieSubmit(e: Event) {
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
</script>

<div class="login-wrapper">
  {#if useWebview}
    <div class="login-form">
      <div class="form-header">
        <h2>{$t("auth.credentials_title")}</h2>
        <p class="hint">{$t("auth.credentials_hint")}</p>
      </div>

      <button class="btn-login" onclick={handleOpenWindow} disabled={$authLoading}>
        {$authLoading ? $t("auth.validating") : $t("auth.login")}
      </button>

      {#if $authMessage}
        <p class="message" class:error={!$isLoggedIn} class:success={$isLoggedIn}>
          {$authMessage}
        </p>
      {/if}

      <button type="button" class="switch-btn" onclick={switchMode} disabled={$authLoading}>
        {$t("auth.use_manual_cookies")}
      </button>
    </div>
  {:else}
    <form class="login-form" onsubmit={handleCookieSubmit}>
      <div class="form-header">
        <h2>{$t("auth.title")}</h2>
        <p class="hint">{$t("auth.hint")}</p>
      </div>

      <label>
        <span>ipb_member_id</span>
        <input type="text" bind:value={ipbMemberId} placeholder="e.g. 1234567" disabled={$authLoading} />
      </label>

      <label>
        <span>ipb_pass_hash</span>
        <input type="password" bind:value={ipbPassHash} placeholder="e.g. abc123def456..." disabled={$authLoading} />
      </label>

      <label>
        <span>igneous</span>
        <input type="password" bind:value={igneous} placeholder="e.g. a1b2c3d4..." disabled={$authLoading} />
      </label>

      <button type="submit" class="btn-login" disabled={$authLoading}>
        {$authLoading ? $t("auth.validating") : $t("auth.login")}
      </button>

      {#if $authMessage}
        <p class="message" class:error={!$isLoggedIn} class:success={$isLoggedIn}>
          {$authMessage}
        </p>
      {/if}

      <button type="button" class="switch-btn" onclick={switchMode} disabled={$authLoading}>
        {$t("auth.use_credentials")}
      </button>
    </form>
  {/if}
</div>

<style>
  .login-wrapper {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    padding: 2rem;
  }

  .login-form {
    width: 100%;
    max-width: 400px;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    background: var(--bg-primary);
    padding: 2.5rem 2rem;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-md);
  }

  .form-header {
    text-align: center;
    margin-bottom: 0.5rem;
  }

  h2 {
    margin: 0 0 0.5rem;
    font-size: 1.1rem;
    font-weight: 700;
  }

  .hint {
    font-size: 0.8rem;
    color: var(--text-muted);
    margin: 0;
    line-height: 1.4;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  label span {
    font-size: 0.75rem;
    font-weight: 600;
    font-family: 'SF Mono', 'Fira Code', monospace;
    color: var(--text-secondary);
  }

  input {
    padding: 0.6rem 0.85rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  input:disabled {
    opacity: 0.4;
  }

  .btn-login {
    padding: 0.65rem 1.2rem;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, box-shadow 0.15s;
    margin-top: 0.25rem;
  }

  .btn-login:hover:not(:disabled) {
    background: var(--accent-hover);
    box-shadow: var(--shadow-sm);
  }

  .btn-login:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .switch-btn {
    padding: 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 0.75rem;
    cursor: pointer;
    text-align: center;
    text-decoration: underline;
    text-underline-offset: 2px;
    transition: color 0.15s;
  }

  .switch-btn:hover:not(:disabled) {
    color: var(--text-secondary);
  }

  .switch-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .message {
    text-align: center;
    padding: 0.5rem 0.75rem;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    margin: 0;
  }

  .message.error {
    background: var(--danger-bg);
    color: var(--red);
  }

  .message.success {
    background: var(--success-bg);
    color: var(--green);
  }
</style>
