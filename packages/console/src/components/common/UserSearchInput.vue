<script setup lang="ts">
import { ref, watch } from "vue";
import UserAvatar from "@/components/common/UserAvatar.vue";

export type UserSuggestion = {
  id: string;
  email: string;
  nickname: string | null;
  qq: string | null;
};

const props = withDefaults(
  defineProps<{
    placeholder?: string;
    loading?: boolean;
    suggestions?: UserSuggestion[];
  }>(),
  {
    placeholder: "搜索用户（邮箱 / 昵称）",
    loading: false,
    suggestions: () => [],
  },
);

const model = defineModel<UserSuggestion | null>({ default: null });

const emit = defineEmits<{ search: [query: string] }>();

const query = ref("");
const open = ref(false);

watch(model, user => {
  query.value = user ? user.nickname || user.email : "";
});

function onInput() {
  model.value = null;
  open.value = true;
  emit("search", query.value);
}

function selectUser(user: UserSuggestion) {
  model.value = user;
  query.value = user.nickname || user.email;
  open.value = false;
}

function onBlur() {
  window.setTimeout(() => {
    open.value = false;
  }, 150);
}
</script>

<template>
  <div class="relative w-full min-w-[200px]">
    <input
      v-model="query"
      type="text"
      class="input w-full"
      :placeholder="placeholder"
      autocomplete="off"
      @input="onInput"
      @focus="open = true"
      @blur="onBlur"
    />
    <ul
      v-if="open && (loading || suggestions.length > 0 || query.trim())"
      class="menu absolute z-20 mt-1 w-full rounded-box bg-base-100 p-1 shadow-lg max-h-56 overflow-y-auto"
    >
      <li v-if="loading" class="px-3 py-2 text-sm text-base-content/60">
        <span class="loading loading-spinner loading-xs mr-2" />
        搜索中…
      </li>
      <li v-else-if="suggestions.length === 0 && query.trim()" class="px-3 py-2 text-sm text-base-content/60">
        未找到匹配用户
      </li>
      <li v-for="user in suggestions" :key="user.id">
        <button type="button" class="flex items-center gap-2 py-2" @mousedown.prevent="selectUser(user)">
          <UserAvatar :qq="user.qq" :size="32" />
          <div class="min-w-0 text-left">
            <div class="truncate text-sm font-medium">{{ user.nickname || user.email }}</div>
            <div class="truncate text-xs text-base-content/60">{{ user.email }}</div>
          </div>
        </button>
      </li>
    </ul>
  </div>
</template>
