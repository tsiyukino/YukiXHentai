<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const appWindow = getCurrentWindow();

  function minimize() {
    appWindow.minimize();
  }

  function toggleMaximize() {
    appWindow.toggleMaximize();
  }

  function close() {
    appWindow.close();
  }
</script>

<div class="window-controls">
  <button class="control-btn" onclick={minimize} title="Minimize">
    <svg width="12" height="12" viewBox="0 0 12 12">
      <rect x="2" y="5.5" width="8" height="1" fill="currentColor" />
    </svg>
  </button>
  <button class="control-btn" onclick={toggleMaximize} title="Maximize">
    <svg width="12" height="12" viewBox="0 0 12 12">
      <rect x="2" y="2" width="8" height="8" rx="1" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
  </button>
  <button class="control-btn close-btn" onclick={close} title="Close">
    <svg width="12" height="12" viewBox="0 0 12 12">
      <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </svg>
  </button>
</div>

<style>
  .window-controls {
    display: flex;
    align-items: center;
    -webkit-app-region: no-drag;
  }

  .control-btn {
    width: 46px;
    height: 32px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.1s, color 0.1s;
  }

  .control-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .close-btn:hover {
    background: var(--red);
    color: #ffffff;
  }
</style>
