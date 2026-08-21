/**
 * useGithubStars —— 实时拉取 GitHub 公开仓库 star 数（真实数据）。
 *
 * - 走 GitHub 公开 API（api.github.com 支持跨域），仓库公开时返回真实 star 数；
 * - 仓库私有 / 不存在（404）/ 网络异常时 available=false，调用方据此隐藏徽标，
 *   绝不显示伪造的 0 或占位数字。
 */
import { ref, onMounted } from "vue";

export function useGithubStars(repo: string) {
  const stars = ref<number | null>(null);
  const loading = ref(true);
  const available = ref(false);

  async function fetchStars() {
    if (!repo) {
      loading.value = false;
      return;
    }
    try {
      const res = await fetch(`https://api.github.com/repos/${repo}`, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!res.ok) {
        available.value = false;
        return;
      }
      const data = (await res.json()) as { stargazers_count?: number };
      if (typeof data.stargazers_count === "number") {
        stars.value = data.stargazers_count;
        available.value = true;
      } else {
        available.value = false;
      }
    } catch {
      available.value = false;
    } finally {
      loading.value = false;
    }
  }

  onMounted(fetchStars);

  return { stars, loading, available };
}

/** 1_250 → "1.3k"，保留真实精度。 */
export function formatStars(n: number): string {
  if (n < 1000) return String(n);
  const k = n / 1000;
  return `${k.toFixed(k >= 10 ? 0 : 1)}k`;
}
