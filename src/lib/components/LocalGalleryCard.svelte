<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { Gallery } from "$lib/api/galleries";
  import { localDetailGallery } from "$lib/stores/localLibrary";
  import GalleryCard from "./GalleryCard.svelte";

  let { gallery }: { gallery: Gallery } = $props();

  let imgSrc = $derived(gallery.thumb_path ? convertFileSrc(gallery.thumb_path) : "");
</script>

<GalleryCard
  {gallery}
  overrideImgSrc={imgSrc}
  noTagNav={true}
  onOpen={(g) => { $localDetailGallery = g; }}
/>
