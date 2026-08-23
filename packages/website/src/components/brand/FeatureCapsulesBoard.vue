<script setup lang="ts">
/**
 * 「春笋能力」特性胶囊板：三行横向无缝滚动 + 两端渐隐。
 */
import { computed, nextTick, onMounted, onUnmounted, ref, useTemplateRef } from "vue";
import gsap from "gsap";
import FeatureCapsule from "@/components/brand/FeatureCapsule.vue";
import type { LucideIcon } from "@lucide/vue";
import {
  Bell,
  Bug,
  Check,
  Compass,
  Database,
  Hash,
  Key,
  Link,
  Lock,
  MessageCircle,
  Monitor,
  Package,
  Pause,
  RefreshCw,
  RotateCw,
  Server,
  Shield,
  Sparkles,
  SquareCheck,
  Zap,
} from "@lucide/vue";

export type FeaturePoint = {
  id: string;
  title: string;
  line: string;
  icon: LucideIcon;
};

/** 产品能力点（事实对齐 README + 长循环技能） */
const features: FeaturePoint[] = [
  {
    id: "self-host",
    title: "自部署",
    line: "单二进制 + PostgreSQL，数据与密钥留在你的实例。",
    icon: Server,
  },
  {
    id: "ssot",
    title: "项目管理",
    line: "需求、轮次、场景以平台为唯一真相源。",
    icon: Monitor,
  },
  {
    id: "handoff",
    title: "跨时空同步",
    line: "换人换会话，进度与决策链在平台无缝接上。",
    icon: RefreshCw,
  },
  {
    id: "agent",
    title: "多 Agent 接入",
    line: "chunsun init 按 IDE 装好技能、命令与规则。",
    icon: Link,
  },
  {
    id: "autonomous",
    title: "自主交付",
    line: "一条 /chunsun，Agent 自主推进到验收绿。",
    icon: Zap,
  },
  {
    id: "memory",
    title: "工作记忆",
    line: "快照与决策留在平台，换会话也能续。",
    icon: Database,
  },
  {
    id: "scenario",
    title: "验收场景",
    line: "场景与用例进平台，不靠口头约定。",
    icon: SquareCheck,
  },
  {
    id: "real-deps",
    title: "真实依赖",
    line: "禁 stub 冒充；真实路径跑通才算 passing。",
    icon: Check,
  },
  {
    id: "defect",
    title: "缺陷闭环",
    line: "/chunsun-fix 派生修复需求并回链缺陷。",
    icon: Bug,
  },
  {
    id: "secrets",
    title: "团队密钥",
    line: "密钥平台加密存储，CLI 实时拉取使用。",
    icon: Key,
  },
  {
    id: "rbac",
    title: "双轨权限",
    line: "平台角色与项目成员分轨授权校验。",
    icon: Lock,
  },
  {
    id: "stop",
    title: "三种停点",
    line: "验收绿、需决策、用户打断——只在此停。",
    icon: Pause,
  },
  {
    id: "rri",
    title: "RRI 反思",
    line: "关键环节评审-反思-改进，reflect 留痕。",
    icon: RotateCw,
  },
  {
    id: "slash",
    title: "斜线极简",
    line: "仅 /chunsun 与 /chunsun-fix 两条命令。",
    icon: Hash,
  },
  {
    id: "steps",
    title: "Step 可追溯",
    line: "think / code / test / verify / ask_user 全程上报。",
    icon: Check,
  },
  {
    id: "gate",
    title: "完成硬门禁",
    line: "场景未全绿，平台拒绝标记 completed。",
    icon: Shield,
  },
  {
    id: "waive",
    title: "自然语言豁免",
    line: "「这个我认了」即可 waived，并留下痕迹。",
    icon: MessageCircle,
  },
  {
    id: "takeover",
    title: "僵尸接管",
    line: "撞锁 Run 可 takeover，避免卡死长循环。",
    icon: Compass,
  },
  {
    id: "remind",
    title: "CLI 提醒",
    line: "remind 柔性约束：补场景、补 reflect、催决策。",
    icon: Bell,
  },
  {
    id: "env",
    title: "环境引用",
    line: "Context 只引用 env key，密钥不进 prompt。",
    icon: Package,
  },
  {
    id: "iterate",
    title: "可再迭代",
    line: "completed 后再 /chunsun，开新 Run 继续生长。",
    icon: Sparkles,
  },
];

/** 均分为三行队列（7 / 7 / 6） */
const rows = computed(() => {
  const per = Math.ceil(features.length / 3);
  return [0, 1, 2].map((i) => features.slice(i * per, (i + 1) * per));
});

const board = useTemplateRef<HTMLElement>("board");
const isStatic = ref(
  typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
);
const setCopies = computed(() => (isStatic.value ? 1 : 3));

type RowRuntime = {
  marquee: HTMLElement;
  track: HTMLElement;
  tween: gsap.core.Tween | null;
  onEnter: () => void;
  onLeave: () => void;
};

const runtimes: RowRuntime[] = [];
let resizeTimer: ReturnType<typeof setTimeout> | null = null;

function prefersReduced() {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function teardownRow(rt: RowRuntime) {
  rt.tween?.kill();
  rt.tween = null;
  rt.marquee.removeEventListener("mouseenter", rt.onEnter);
  rt.marquee.removeEventListener("mouseleave", rt.onLeave);
  gsap.set(rt.track, { clearProps: "transform" });
}

function teardownAll() {
  for (const rt of runtimes) teardownRow(rt);
  runtimes.length = 0;
}

function setupRow(marquee: HTMLElement, index: number) {
  const track = marquee.querySelector<HTMLElement>(".capsule-track");
  const set = marquee.querySelector<HTMLElement>(".capsule-set");
  if (!track || !set) return;

  const period = set.getBoundingClientRect().width;
  if (!period || period <= 0) return;

  const reverse = index % 2 === 1;
  const pxPerSec = 42 + index * 6;
  const duration = Math.max(22, period / pxPerSec);

  const onEnter = () => rt.tween?.pause();
  const onLeave = () => rt.tween?.play();

  const rt: RowRuntime = { marquee, track, tween: null, onEnter, onLeave };

  if (reverse) {
    gsap.set(track, { x: -period });
    rt.tween = gsap.to(track, {
      x: 0,
      ease: "none",
      duration,
      repeat: -1,
    });
  } else {
    gsap.set(track, { x: 0 });
    rt.tween = gsap.to(track, {
      x: -period,
      ease: "none",
      duration,
      repeat: -1,
    });
  }

  marquee.addEventListener("mouseenter", onEnter);
  marquee.addEventListener("mouseleave", onLeave);
  runtimes.push(rt);
}

async function setupAll() {
  teardownAll();
  if (!board.value || prefersReduced()) return;

  await nextTick();
  // 等一帧，确保 flex 宽度稳定后再量周期
  await new Promise<void>((r) => requestAnimationFrame(() => r()));

  if (!board.value) return;
  const marquees = Array.from(
    board.value.querySelectorAll<HTMLElement>(".capsule-marquee")
  );
  marquees.forEach((el, i) => setupRow(el, i));
}

function onResize() {
  if (resizeTimer) clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => void setupAll(), 140);
}

onMounted(() => {
  isStatic.value = prefersReduced();
  void setupAll();
  window.addEventListener("resize", onResize, { passive: true });
});

onUnmounted(() => {
  if (resizeTimer) clearTimeout(resizeTimer);
  window.removeEventListener("resize", onResize);
  teardownAll();
});
</script>

<template>
  <div
    ref="board"
    class="feature-capsules-board"
    :class="{ 'is-static': isStatic }"
  >
    <div
      v-for="(row, rowIndex) in rows"
      :key="'row-' + rowIndex"
      class="capsule-marquee"
    >
      <div class="capsule-track">
        <div
          v-for="copy in setCopies"
          :key="'set-' + rowIndex + '-' + copy"
          class="capsule-set"
          :role="copy === 1 ? 'list' : undefined"
          :aria-hidden="copy > 1 ? 'true' : undefined"
        >
          <FeatureCapsule
            v-for="f in row"
            :key="copy + '-' + f.id"
            :role="copy === 1 ? 'listitem' : undefined"
            :title="f.title"
            :line="f.line"
            :icon="f.icon"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.feature-capsules-board {
  display: grid;
  gap: 0.85rem;
  width: 100%;
}

.capsule-marquee {
  overflow: hidden;
  -webkit-mask-image: linear-gradient(
    90deg,
    transparent,
    #000 8%,
    #000 92%,
    transparent
  );
  mask-image: linear-gradient(90deg, transparent, #000 8%, #000 92%, transparent);
}

.capsule-track {
  display: flex;
  align-items: center;
  width: max-content;
  will-change: transform;
}

.capsule-set {
  display: flex;
  align-items: stretch;
  gap: 0.85rem;
  flex-shrink: 0;
  padding-block: 0.1rem;
}

/* 末项右侧补同等 gap，使一组宽度自包含，无缝衔接无断层 */
.capsule-set :deep(.feature-capsule:last-child) {
  margin-inline-end: 0.85rem;
}

/* 降动效：静态换行平铺，不做走马灯 */
.feature-capsules-board.is-static .capsule-marquee {
  overflow: visible;
  -webkit-mask-image: none;
  mask-image: none;
}

.feature-capsules-board.is-static .capsule-track {
  width: auto;
  flex-wrap: wrap;
  justify-content: center;
  row-gap: 0.75rem;
}

.feature-capsules-board.is-static .capsule-set:not(:first-child) {
  display: none;
}

.feature-capsules-board.is-static .capsule-set {
  flex-wrap: wrap;
  justify-content: center;
  width: 100%;
}

.feature-capsules-board.is-static .capsule-set :deep(.feature-capsule:last-child) {
  margin-inline-end: 0;
}
</style>
