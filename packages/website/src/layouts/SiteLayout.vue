<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { BrandMark } from "@chunsun/web-shared";
import { useGithubStars, formatStars } from "@/composables/useGithubStars";
import { useSiteToast } from "@/composables/useSiteToast";

const route = useRoute();
const router = useRouter();
const toast = useSiteToast();

const GITHUB_REPO = "mqhe2007/chunsun";
const GITHUB_URL = `https://github.com/${GITHUB_REPO}`;

const { stars, available } = useGithubStars(GITHUB_REPO);

function goToLogin() {
  location.assign("/console/auth/login");
}

function goDocs() {
  router.push("/docs");
}

function showComingSoon() {
  toast.add({
    severity: "info",
    summary: "快要破土，敬请期待",
    life: 2500,
  });
}

function goHome() {
  if (route.path === "/") {
    window.scrollTo({ top: 0, behavior: "smooth" });
  } else {
    router.push("/");
  }
}

/** 当前是否在文档页（用于导航高亮） */
const isDocsActive = computed(() => route.path.startsWith("/docs"));
</script>

<template>
  <div class="site-layout site-theme">
    <header class="site-nav">
      <div class="site-rail site-nav-inner">
        <!-- 左：品牌 + GitHub star 真实数据 -->
        <a class="nav-logo" @click.prevent="goHome">
          <BrandMark size="2.25rem" />
          <span class="nav-logo-text">春笋</span>
          <a
            v-if="available && stars !== null"
            class="nav-star"
            :href="GITHUB_URL"
            target="_blank"
            rel="noopener noreferrer"
            :title="`在 GitHub 上为 ${GITHUB_REPO} 加星（${stars} stars）`"
          >
            <svg
              class="nav-star-icon"
              viewBox="0 0 24 24"
              width="14"
              height="14"
              aria-hidden="true"
            >
              <path
                d="M12 2.5l2.95 5.98 6.6.96-4.77 4.65 1.13 6.57L12 17.98 6.09 20.66l1.13-6.57L2.45 9.44l6.6-.96L12 2.5z"
              />
            </svg>
            <span class="nav-star-count">{{ formatStars(stars) }}</span>
          </a>
        </a>

        <!-- 右：统一尺寸的按钮组（文档→真实页面，案例 敬请期待，继续生长=绿色→登录） -->
        <div class="nav-actions">
          <button
            type="button"
            class="site-btn site-btn-ghost nav-action"
            :class="{ 'nav-action-active': isDocsActive }"
            @click="goDocs"
          >
            文档
          </button>
          <button type="button" class="site-btn site-btn-ghost nav-action" @click="showComingSoon">
            案例
          </button>
          <button type="button" class="site-btn site-btn-primary nav-action nav-action-primary" @click="goToLogin">
            继续生长
          </button>
        </div>
      </div>
    </header>
    <slot />
  </div>
</template>

<style scoped>
.site-layout {
  min-height: 100vh;
  background: var(--chunsun-fog);
  color: var(--chunsun-ink);
  /* 横向裁剪必须用 clip：hidden 会把 overflow-y 联动计算成 auto，
     使本元素成为滚动容器，导致内部 position: sticky 相对一个不滚动的
     scrollport 定位而完全失效（journey 区钉住失效、滚动后留白板）。
     clip 不产生 scrollport，视觉效果与 hidden 相同（本容器从不内部滚动）。 */
  overflow-x: clip;
}

.site-nav {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 1000;
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  padding-block: 0.95rem;
}

.site-nav-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

/* ── 左：品牌 ── */
.nav-logo {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  cursor: pointer;
  text-decoration: none;
}

.nav-logo-text {
  font-weight: 700;
  font-size: 1.35rem;
  letter-spacing: -0.02em;
  color: var(--chunsun-ink);
}

/* GitHub star 徽标 */
.nav-star {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  margin-left: 0.2rem;
  padding: 0.2rem 0.5rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--chunsun-shoot) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 22%, transparent);
  color: var(--chunsun-node);
  font-size: 0.78rem;
  font-weight: 600;
  line-height: 1;
  text-decoration: none;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
}

.nav-star:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 18%, transparent);
  border-color: color-mix(in srgb, var(--chunsun-shoot) 38%, transparent);
  transform: translateY(-1px);
}

.nav-star-icon {
  fill: var(--chunsun-shoot);
  flex-shrink: 0;
}

.nav-star-count {
  font-variant-numeric: tabular-nums;
}

/* ── 右：按钮组（统一尺寸） ── */
.nav-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.nav-action:not(.nav-action-primary) {
  color: var(--chunsun-ink);
  font-weight: 500;
}

.nav-action:not(.nav-action-primary):hover {
  background: color-mix(in srgb, var(--chunsun-ink) 7%, transparent);
}

.nav-action.nav-action-active {
  color: var(--chunsun-node);
  font-weight: 600;
  background: color-mix(in srgb, var(--chunsun-shoot) 10%, transparent);
}

.nav-action.nav-action-active:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 14%, transparent);
}

@media (max-width: 640px) {
  .nav-actions .nav-action:not(.nav-action-primary) {
    display: none;
  }
}
</style>
