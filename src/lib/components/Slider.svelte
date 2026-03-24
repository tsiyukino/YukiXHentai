<script lang="ts">
  interface Props {
    min?: number;
    max?: number;
    step?: number;
    value: number;
    disabled?: boolean;
    onChange?: (value: number) => void;
  }

  let { min = 0, max = 100, step = 1, value = $bindable(), disabled = false, onChange }: Props = $props();

  function handleInput(e: Event) {
    const v = parseInt((e.target as HTMLInputElement).value);
    value = v;
    onChange?.(v);
  }

  // Compute fill percentage for the track
  let fillPct = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="slider-wrap" class:disabled>
  <input
    type="range"
    {min}
    {max}
    {step}
    {value}
    {disabled}
    oninput={handleInput}
    style="--fill: {fillPct}%"
  />
</div>

<style>
  .slider-wrap {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 9px;
    border-radius: 9px;
    outline: none;
    cursor: pointer;
    border: none;
    padding: 0;
    background: linear-gradient(
      to right,
      var(--accent) 0%,
      var(--accent) var(--fill),
      var(--slider-track, var(--bg-elevated)) var(--fill),
      var(--slider-track, var(--bg-elevated)) 100%
    );
    transition: background 0s;
  }

  input[type="range"]:focus-visible {
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  /* WebKit thumb */
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--bg-primary);
    border: 2.5px solid var(--accent);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
    cursor: pointer;
    transition: border-color 0.1s, box-shadow 0.1s;
  }

  input[type="range"]:hover::-webkit-slider-thumb {
    border-color: var(--accent-hover);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.22);
  }

  input[type="range"]:active::-webkit-slider-thumb {
    box-shadow: 0 0 0 4px var(--accent-subtle), 0 2px 8px rgba(0, 0, 0, 0.22);
  }

  /* Firefox thumb */
  input[type="range"]::-moz-range-thumb {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--bg-primary);
    border: 2.5px solid var(--accent);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
    cursor: pointer;
    transition: border-color 0.1s, box-shadow 0.1s;
  }

  input[type="range"]:hover::-moz-range-thumb {
    border-color: var(--accent-hover);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.22);
  }

  /* Firefox track (filled portion handled via background on input) */
  input[type="range"]::-moz-range-track {
    background: transparent;
    height: 9px;
    border-radius: 9px;
  }

  .disabled input[type="range"] {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
