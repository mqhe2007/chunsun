<template>
  <div class="drawer lg:drawer-open min-h-dvh">
    <input :id="drawerId" v-model="drawerOpen" type="checkbox" class="drawer-toggle" />

    <div class="drawer-content flex h-dvh min-h-0 min-w-0 flex-col overflow-hidden bg-base-200">
      <header class="navbar min-h-12 px-3 lg:hidden bg-base-200">
        <div class="navbar-start">
          <label
            :for="drawerId"
            class="btn btn-square btn-ghost drawer-button"
            aria-label="打开菜单"
          >
            <Menu class="h-5 w-5" />
          </label>
        </div>
        <div class="navbar-center flex items-center gap-2">
          <span class="font-semibold">春笋</span>
          <span class="badge badge-ghost badge-sm">v{{ appVersion }}</span>
        </div>
        <div class="navbar-end" />
      </header>

      <main class="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto p-4 sm:p-6 lg:p-8 has-[.req-page--board]:overflow-hidden">
        <slot />
      </main>
    </div>

    <div class="drawer-side z-40 overflow-visible">
      <label :for="drawerId" class="drawer-overlay" aria-label="关闭菜单" />
      <aside class="bg-base-100 min-h-full w-60 flex flex-col text-base-content">
        <RouterLink
          to="/projects"
          class="flex h-14 shrink-0 items-center gap-2 px-4 text-base-content"
          aria-label="回到项目管理"
          @click="closeDrawer"
        >
          <BrandMark :size="24" />
          <span class="font-semibold tracking-tight">春笋</span>
          <span class="badge badge-ghost badge-sm">v{{ appVersion }}</span>
        </RouterLink>

        <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
          <slot name="nav" :close-drawer="closeDrawer" />
        </div>

        <div class="shrink-0 p-2">
          <AppDropdown v-model:open="userMenuOpen" class="dropdown-top dropdown-start w-full" :items="userMenuItems">
            <template #trigger="{ toggle }">
              <button
                type="button"
                class="btn btn-ghost h-auto min-h-0 w-full justify-start gap-3 rounded-box bg-base-200 px-2 py-2"
                aria-label="账户菜单"
                aria-haspopup="true"
                @click="toggle"
              >
                <UserAvatar :qq="profile?.qq" :size="32" rounded />
                <span class="min-w-0 flex-1 text-start">
                  <span class="block truncate text-sm font-medium">{{ displayName }}</span>
                  <span v-if="userEmail" class="block truncate text-xs text-base-content/50">
                    {{ userEmail }}
                  </span>
                </span>
              </button>
            </template>
          </AppDropdown>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Menu } from "@lucide/vue";
import { ref } from "vue";
import { AppDropdown } from "@/ui";
import { BrandMark } from "@chunsun/web-shared";
import UserAvatar from "@/components/common/UserAvatar.vue";
import { useUserMenu } from "@/composables/useUserMenu";

const props = defineProps<{
  drawerId: string;
  includeSystemAdmin: boolean;
}>();

const appVersion = __APP_VERSION__;
const drawerOpen = ref(false);
const userMenuOpen = ref(false);

const { profile, displayName, userEmail, userMenuItems } = useUserMenu(props.includeSystemAdmin);

function closeDrawer() {
  drawerOpen.value = false;
}
</script>
