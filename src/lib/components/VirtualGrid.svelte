<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  // Footer is rendered as an extra row with a small fixed height (not the full rowStride).
  const FOOTER_HEIGHT = 64;

  let {
    items,
    rowHeight,
    columnMinWidth,
    gap = 0,
    buffer = 8,
    onScrollNearEnd,
    onVisibleRangeChanged,
    scrollEndThreshold = 200,
    recheckTrigger = 0,
    children,
    footer,
  }: {
    items: any[];
    rowHeight: number;
    columnMinWidth: number;
    gap?: number;
    buffer?: number;
    onScrollNearEnd?: () => void;
    onVisibleRangeChanged?: (startIndex: number, endIndex: number) => void;
    scrollEndThreshold?: number;
    recheckTrigger?: number;
    children: (item: any, index: number) => any;
    footer?: () => any;
  } = $props();

  let container: HTMLDivElement | undefined = $state(undefined);
  let scrollTop = $state(0);
  let containerWidth = $state(0);
  let containerHeight = $state(0);

  let gridWidth = $derived(Math.max(0, containerWidth - 64)); // subtract 2rem padding on each side
  let columns = $derived(Math.max(1, Math.floor((gridWidth + gap) / (columnMinWidth + gap))));
  // Number of rows containing actual data items.
  let dataRows = $derived(Math.ceil(items.length / columns));
  let rowStride = $derived(rowHeight + gap);
  // Total virtual rows: data rows + 1 footer row (when footer snippet provided).
  let totalRows = $derived(dataRows + (footer ? 1 : 0));
  // Total spacer height: data rows at full rowStride + optional small footer + padding.
  let spacerHeight = $derived.by(() => {
    if (dataRows === 0 && !footer) return 0;
    const dataHeight = dataRows > 0 ? dataRows * rowStride - gap : 0;
    const footerH = footer ? FOOTER_HEIGHT + gap : 0;
    return dataHeight + footerH + 64; // +64 for 2rem padding top+bottom
  });

  let adjustedScrollTop = $derived(Math.max(0, scrollTop - 32)); // subtract top padding
  let startRow = $derived(Math.max(0, Math.floor(adjustedScrollTop / rowStride) - buffer));
  let endRow = $derived(Math.min(totalRows, Math.ceil((adjustedScrollTop + containerHeight) / rowStride) + buffer));

  // Visible data items (excludes the footer row — that's handled separately in the template).
  let visibleItems = $derived.by(() => {
    const dataStartRow = Math.min(startRow, dataRows);
    const dataEndRow = Math.min(endRow, dataRows);
    const startIdx = dataStartRow * columns;
    const endIdx = Math.min(items.length, dataEndRow * columns);
    const result: { item: any; index: number }[] = [];
    for (let i = startIdx; i < endIdx; i++) {
      result.push({ item: items[i], index: i });
    }
    return result;
  });

  // Notify parent about visible range changes (for viewport-driven thumbnail loading).
  $effect(() => {
    if (!onVisibleRangeChanged) return;
    const dataStartRow = Math.min(startRow, dataRows);
    const dataEndRow = Math.min(endRow, dataRows);
    const startIdx = dataStartRow * columns;
    const endIdx = Math.min(items.length, dataEndRow * columns);
    onVisibleRangeChanged(startIdx, endIdx);
  });

  // Is the footer row within the visible range?
  let footerVisible = $derived(footer != null && endRow > dataRows && startRow <= dataRows);
  // Footer Y position: sits right after the last data row.
  let footerY = $derived((dataRows > 0 ? dataRows * rowStride - gap : 0) + gap + 32); // +32 for top padding

  let offsetY = $derived(Math.min(startRow, dataRows) * rowStride + 32); // +32px for 2rem top padding

  // Recheck scroll proximity after items change or sync completes.
  // Captures scroll position synchronously (before Svelte updates the spacer height),
  // then uses double-rAF to measure the DOM after layout.
  $effect(() => {
    const len = items.length;
    const trigger = recheckTrigger;
    if (!container || !onScrollNearEnd) return;
    // Capture current scroll position synchronously. When items are appended,
    // the spacer height grows but scrollTop stays the same — so by the time
    // the rAF fires, the distance-from-bottom would be large. Instead, we
    // snapshot the distance NOW (before layout update) to detect if the user
    // was at/near the bottom before the append.
    const snapshotDist = Math.ceil(container.scrollHeight - container.scrollTop - container.clientHeight);
    // Double-rAF: first rAF runs before paint, second runs after layout.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!container || !onScrollNearEnd) return;
        const distFromBottom = Math.ceil(container.scrollHeight - container.scrollTop - container.clientHeight);
        // Trigger if near the bottom now, OR if the user was near the bottom
        // before the items changed (the spacer grew but user didn't scroll).
        if (distFromBottom <= scrollEndThreshold || snapshotDist <= scrollEndThreshold) {
          onScrollNearEnd();
        }
      });
    });
  });

  let observer: ResizeObserver | null = null;

  onMount(() => {
    if (container) {
      containerWidth = container.clientWidth;
      containerHeight = container.clientHeight;

      observer = new ResizeObserver((entries) => {
        for (const entry of entries) {
          containerWidth = entry.contentRect.width;
          containerHeight = entry.contentRect.height;
        }
      });
      observer.observe(container);
    }
  });

  onDestroy(() => {
    observer?.disconnect();
    if (rafId) cancelAnimationFrame(rafId);
  });

  let rafId = 0;

  function handleScroll(e: Event) {
    if (rafId) return;
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      const target = e.target as HTMLElement;
      scrollTop = target.scrollTop;

      if (onScrollNearEnd) {
        const distFromBottom = Math.ceil(target.scrollHeight - target.scrollTop - target.clientHeight);
        if (distFromBottom <= scrollEndThreshold) {
          onScrollNearEnd();
        }
      }
    });
  }
</script>

<div class="virtual-grid-scroll" bind:this={container} onscroll={handleScroll}>
  <div class="virtual-grid-spacer" style="height:{spacerHeight}px">
    <div
      class="virtual-grid-visible"
      style="transform:translateY({offsetY}px);grid-template-columns:repeat(auto-fill, minmax({columnMinWidth}px, 1fr));gap:{gap}px"
    >
      {#each visibleItems as { item, index } (item?.gid ?? index)}
        {@render children(item, index)}
      {/each}
    </div>
    {#if footerVisible}
      <div class="virtual-grid-footer" style="transform:translateY({footerY}px);height:{FOOTER_HEIGHT}px">
        {@render footer()}
      </div>
    {/if}
  </div>
</div>

<style>
  .virtual-grid-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    contain: strict;
  }

  .virtual-grid-spacer {
    position: relative;
    width: 100%;
  }

  .virtual-grid-visible {
    display: grid;
    position: absolute;
    left: 0;
    right: 0;
    padding: 0 2rem;
    will-change: transform;
  }

  .virtual-grid-footer {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    justify-content: center;
    align-items: center;
    will-change: transform;
  }
</style>
