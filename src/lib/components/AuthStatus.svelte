<script lang="ts">
  import { t } from "$lib/i18n";
  import { logout } from "$lib/api/auth";
  import { isLoggedIn, authLoading, authMessage } from "$lib/stores/auth";

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
</script>

<div class="auth-status">
  <span class="indicator"></span>
  <span class="label">{$t("auth.connected")}</span>
  <button onclick={handleLogout} disabled={$authLoading}>
    {$authLoading ? "..." : $t("auth.logout")}
  </button>
</div>

<style>
  .auth-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
  }

  .indicator {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--green);
  }

  .label {
    color: var(--text-muted);
    font-size: 0.75rem;
  }

  button {
    padding: 0.25rem 0.6rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--danger-border);
    background: transparent;
    color: var(--red);
    font-size: 0.7rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  button:hover:not(:disabled) {
    background: var(--danger-bg);
  }

  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
