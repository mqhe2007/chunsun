<script setup lang="ts">
import { useId } from "vue";
import { useRouter } from "vue-router";
import type { LucideIcon } from "@lucide/vue";

export type MegaNavItem = {
  key: string;
  label: string;
  to?: string;
  icon?: LucideIcon;
};

defineProps<{
  items: MegaNavItem[];
  activeKey: string;
  label: string;
}>();

const emit = defineEmits<{
  select: [key: string];
}>();

const router = useRouter();
const uid = useId();

function popoverId(key: string) {
  return `${uid}-${key || "root"}`;
}

function closePopover(key: string) {
  document.getElementById(popoverId(key))?.hidePopover();
}

function go(item: MegaNavItem) {
  emit("select", item.key);
  if (item.to) void router.push(item.to);
  requestAnimationFrame(() => closePopover(item.key));
}
</script>

<template>
  <nav :aria-label="label">
    <div class="megamenu flex-wrap rounded-box bg-base-100 p-2">
      <span class="megamenu-active" />
      <template v-for="item in items" :key="item.key || 'root'">
        <button
          type="button"
          class="after:content-none"
          :class="{ 'text-primary': activeKey === item.key }"
          :popovertarget="popoverId(item.key)"
          :aria-current="activeKey === item.key ? 'true' : undefined"
          @click="go(item)"
        >
          <component :is="item.icon" v-if="item.icon" :size="14" aria-hidden="true" />
          {{ item.label }}
        </button>
        <div :id="popoverId(item.key)" popover="auto">
          <ul class="menu">
            <li>
              <RouterLink
                v-if="item.to"
                :to="item.to"
                :class="{ 'menu-active': activeKey === item.key }"
                @click="closePopover(item.key)"
              >
                <component :is="item.icon" v-if="item.icon" :size="14" aria-hidden="true" />
                {{ item.label }}
              </RouterLink>
              <button
                v-else
                type="button"
                :class="{ 'menu-active': activeKey === item.key }"
                @click="go(item)"
              >
                <component :is="item.icon" v-if="item.icon" :size="14" aria-hidden="true" />
                {{ item.label }}
              </button>
            </li>
          </ul>
        </div>
      </template>
    </div>
  </nav>
</template>
