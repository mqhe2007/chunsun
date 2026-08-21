import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "../utils/api";

type SetupStatus = {
  needed: boolean;
  listenPort: number;
};

export const useSetupStore = defineStore("setup", () => {
  const needed = ref<boolean | null>(null);
  const loaded = ref(false);

  async function refresh(): Promise<boolean> {
    const res = await api.get<{ success: boolean; data: SetupStatus }>("/setup/status");
    needed.value = Boolean(res.data.data.needed);
    loaded.value = true;
    return needed.value;
  }

  function markComplete() {
    needed.value = false;
    loaded.value = true;
  }

  return { needed, loaded, refresh, markComplete };
});
