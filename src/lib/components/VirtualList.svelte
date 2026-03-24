<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  // Footer is rendered as an extra item with a small fixed height (not the full rowStride).
  const FOOTER_HEIGHT = 64;

  let {
    items,
    rowHeight,
    gap = 0,
    buffer = 20,
    onScrollNearEnd,
    onVisibleRangeChanged,
    scrollEndThreshold = 200,
    recheckTrigger = 0,
    children,
    footer,
  }: {
    items: any[];
    rowHeight: number;
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
  let containerHeight = $state(0);

  let rowStride = $derived(rowHeight + gap);
  // Number of actual data items.
  let dataCount = $derived(items.length);
  // Total virtual items: data + 1 footer (when footer snippet provided).
  let totalCount = $derived(dataCount + (footer ? 1 : 0));
  // Spacer height: data items at full rowStride + optional small footer + padding.
  let spacerHeight = $derived.by(() => {
    if (dataCount === 0 && !footer) return 0;
    const dataHeight = dataCount > 0 ? dataCount * rowStride - gap : 0;
    const footerH = footer ? FOOTER_HEIGHT + gap : 0;
    return dataHeight + footerH + 64; // +64 for 2rem padding top+bottom
  });

  let adjustedScrollTop = $derived(Math.max(0, scrollTop - 32));
  let startIdx = $derived(Math.max(0, Math.floor(adjustedScrollTop / rowStride) - buffer));
  let endIdx = $derived(Math.min(totalCount, Math.ceil((adjustedScrollTop + containerHeight) / rowStride) + buffer));

  // Visible data items (excludes the footer — handled separately).
  let visibleItems = $derived.by(() => {
    const dataStart = Math.min(startIdx, dataCount);
    const dataEnd = Math.min(endIdx, dataCount);
    const result: { item: any; index: number }[] = [];
    for (let i = dataStart; i < dataEnd; i++) {
      result.push({ item: items[i], index: i });
    }
    return result;
  });

  // Notify parent about visible range changes (for viewport-driven thumbnail loading).
  $effect(() => {
    if (!onVisibleRangeChanged) return;
    const dataStart = Math.min(startIdx, dataCount);
    const dataEnd = Math.min(endIdx, dataCount);
    onVisibleRangeChanged(dataStart, dataEnd);
  });

  // Is the footer within the visible range?
  let footerVisible = $derived(footer != null && endIdx > dataCount && startIdx <= dataCount);
  // Footer Y position: sits right after the last data item.
  let footerY = $derived((dataCount > 0 ? dataCount * rowStride - gap : 0) + gap + 32); // +32 for top padding

  let offsetY = $derived(Math.min(startIdx, dataCount) * rowStride + 32);

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
      containerHeight = container.clientHeight;
      observer = new ResizeObserver((entries) => {
        for (const entry of entries) {
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

<div class="virtual-list-scroll" bind:this={container} onscroll={handleScroll}>
  <div class="virtual-list-spacer" style="height:{spacerHeight}px">
    <div class="virtual-list-visible" style="transform:translateY({offsetY}px);gap:{gap}px">
      {#each visibleItems as { item, index } (item?.gid ?? index)}
        {@render children(item, index)}
      {/each}
    </div>
    {#if footerVisible}
      <div class="virtual-list-footer" style="transform:translateY({footerY}px);height:{FOOTER_HEIGHT}px">
        {@render footer()}
      </div>
    {/if}
  </div>
</div>

<style>
  .virtual-list-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    contain: strict;
  }

  .virtual-list-spacer {
    position: relative;
    width: 100%;
  }

  .virtual-list-visible {
    display: flex;
    flex-direction: column;
    position: absolute;
    left: 0;
    right: 0;
    padding: 0 2rem;
    will-change: transform;
  }

  .virtual-list-footer {
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
