<script setup lang="ts">
import { onBeforeMount, ref } from "vue";
import SiteToastHost from "./components/SiteToastHost.vue";

const ready = ref(false);

onBeforeMount(async () => {
  try {
    const res = await fetch("/api/v1/setup/status");
    if (!res.ok) {
      location.assign("/console/setup");
      return;
    }
    const body = (await res.json()) as { data?: { needed?: boolean } };
    if (body?.data?.needed) {
      location.assign("/console/setup");
      return;
    }
  } catch {
    location.assign("/console/setup");
    return;
  }
  ready.value = true;
});
</script>

<template>
  <template v-if="ready">
    <router-view />
    <SiteToastHost />
  </template>
</template>
