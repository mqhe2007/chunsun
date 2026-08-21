<script setup lang="ts">
import { Check, CornerDownLeft } from "@lucide/vue";
import { useTemplateRef, ref, computed, nextTick, onMounted, onBeforeUnmount } from "vue";
import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { BrandMark } from "@chunsun/web-shared";
import FeatureCapsulesBoard from "@/components/brand/FeatureCapsulesBoard.vue";
import HeroBambooScene from "@/components/brand/HeroBambooScene.vue";
import JourneyProductMock from "@/components/brand/JourneyProductMock.vue";
import type { JourneyMock, JourneyPhase } from "@/components/brand/JourneyProductMock.vue";
import { useLandingReveal } from "@/composables/useLandingReveal";

const pageRoot = useTemplateRef<HTMLElement>("pageRoot");
useLandingReveal(pageRoot);

const appVersion = __APP_VERSION__;
const GITHUB_URL = "https://github.com/mqhe2007/chunsun";
const LICENSE_URL = `${GITHUB_URL}/blob/main/LICENSE`;

function goToRegister() {
  location.assign("/console/auth/register");
}

/* ── 已支持的 Agent（事实来源：packages/cli/src/ide.rs 的 IDE_TARGETS）── */
const supportedAgents = [
  { name: "Cursor", logo: "/agents/cursor.svg" },
  { name: "Trae", logo: "/agents/trae.png" },
  { name: "Qoder", logo: "/agents/qoder.svg" },
  { name: "CodeBuddy", logo: "/agents/codebuddy.svg" },
  { name: "WorkBuddy", logo: "/agents/workbuddy.svg" },
  { name: "Claude Code", logo: "/agents/claude-code.svg" },
  { name: "CodeWhale", logo: "/agents/codewhale.svg" },
  { name: "DeepSeek Harness", logo: "/agents/deepseek-harness.svg" },
];

type JourneyStep = {
  id: JourneyPhase;
  t: string;
  b: string;
  mock: JourneyMock;
};

const journeySteps: JourneyStep[] = [
  {
    id: "write",
    t: "写下",
    b: "验收场景与边界一起进平台，不靠口头约定。",
    mock: {
      statusLabel: "待处理",
      stats: [
        { k: "Run 次数", v: "0" },
        { k: "最新 Run", v: "—" },
        { k: "场景通过", v: "0/3" },
        { k: "未决决策", v: "0" },
      ],
      runTitle: "交付轮次",
      runStatus: "",
      steps: [],
      emptyRun: "尚未发起——用 /chunsun 启动",
      scenarios: [
        { title: "首屏品牌与 CTA 可见", status: "pending" },
        { title: "自主交付演示到达验收绿", status: "pending" },
        { title: "旅程四步滚动同步", status: "pending" },
      ],
    },
  },
  {
    id: "run",
    t: "推进",
    b: "自主交付自己跑；你只在断点做决策。",
    mock: {
      statusLabel: "运行中",
      stats: [
        { k: "Run 次数", v: "1" },
        { k: "最新 Run", v: "运行中" },
        { k: "场景通过", v: "1/3" },
        { k: "未决决策", v: "0" },
      ],
      runTitle: "#1 · 交付轮次",
      runStatus: "运行中",
      steps: [
        { kind: "思考", summary: "规划区段与验收用例", done: true },
        { kind: "编码", summary: "实现营销首页 HomeView", done: true },
        { kind: "测试", summary: "真实浏览器验收进行中", active: true },
        { kind: "反思", summary: "评审 · 反思 · 改进" },
      ],
      scenarios: [
        { title: "首屏品牌与 CTA 可见", status: "passing" },
        { title: "自主交付演示到达验收绿", status: "pending" },
        { title: "旅程四步滚动同步", status: "pending" },
      ],
    },
  },
  {
    id: "resume",
    t: "续跑",
    b: "进度与决策留在平台，换会话、换人可接。",
    mock: {
      statusLabel: "已结束",
      stats: [
        { k: "Run 次数", v: "1" },
        { k: "最新 Run", v: "已结束" },
        { k: "场景通过", v: "2/3" },
        { k: "未决决策", v: "1", warn: true },
      ],
      runTitle: "#1 · 交付轮次",
      runStatus: "已结束",
      runReason: "结束原因：等待你确认窄屏策略",
      steps: [
        { kind: "思考", summary: "规划区段与验收用例", done: true },
        { kind: "编码", summary: "实现营销首页 HomeView", done: true },
        { kind: "测试", summary: "真实浏览器验收 2/3", done: true },
        { kind: "询问", summary: "等待你确认窄屏策略", active: true },
      ],
      scenarios: [
        { title: "首屏品牌与 CTA 可见", status: "passing" },
        { title: "自主交付演示到达验收绿", status: "passing" },
        { title: "旅程四步滚动同步", status: "blocked" },
      ],
      decision: "窄屏是否保留滚动钉住，还是改为静态时间轴？",
    },
  },
  {
    id: "done",
    t: "完成",
    b: "场景全绿，需求完成；缺陷可回链到源头。",
    mock: {
      statusLabel: "已完成",
      stats: [
        { k: "Run 次数", v: "1" },
        { k: "最新 Run", v: "已完成" },
        { k: "场景通过", v: "3/3" },
        { k: "未决决策", v: "0" },
      ],
      runTitle: "#1 · 交付轮次",
      runStatus: "已完成",
      steps: [
        { kind: "思考", summary: "规划区段与验收用例", done: true },
        { kind: "编码", summary: "实现营销首页 HomeView", done: true },
        { kind: "测试", summary: "真实浏览器验收 3/3", done: true },
        { kind: "反思", summary: "评审 · 反思 · 改进", done: true },
      ],
      scenarios: [
        { title: "首屏品牌与 CTA 可见", status: "passing" },
        { title: "自主交付演示到达验收绿", status: "passing" },
        { title: "旅程四步滚动同步", status: "passing" },
      ],
      footer: "验收全绿 → 需求完成",
    },
  },
];

/* ── Agent Chat：自主交付演示（循环播放） ── */
type ChatRole = "user" | "agent" | "system";
type ChatStep = "think" | "code" | "test" | "reflect";

type ChatMsg = {
  role: ChatRole;
  step?: ChatStep;
  text: string;
};

const chatScript: ChatMsg[] = [
  { role: "user", text: "/chunsun JD1XwcSARhGf" },
  { role: "agent", step: "think", text: "规划区段与验收用例" },
  { role: "agent", step: "code", text: "实现营销首页 HomeView.vue" },
  { role: "agent", step: "test", text: "真实浏览器验收 16/16 通过" },
  { role: "agent", step: "reflect", text: "评审 · 反思 · 改进" },
  { role: "system", text: "验收全绿 → 需求完成" },
];

const visibleMessages = ref<(ChatMsg & { key: string })[]>([]);
const chatTyping = ref(false);
const chatCycle = ref(0);
const chatAnimLive = ref(false);
const chatBodyEl = useTemplateRef<HTMLElement>("chatBody");
let chatAbort = false;

function sleep(ms: number) {
  return new Promise<void>((r) => setTimeout(r, ms));
}

async function sleepAbortable(ms: number) {
  const step = 80;
  let left = ms;
  while (left > 0) {
    if (chatAbort) return false;
    const slice = Math.min(step, left);
    await sleep(slice);
    left -= slice;
  }
  return !chatAbort;
}

function scrollChatToBottom() {
  const el = chatBodyEl.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

async function runChatLoop() {
  if (reduceMotion) {
    visibleMessages.value = chatScript.map((m, i) => ({ ...m, key: `static-${i}` }));
    chatTyping.value = false;
    chatCycle.value = 1;
    return;
  }

  let seq = 0;
  while (!chatAbort) {
    visibleMessages.value = [];
    chatTyping.value = false;
    if (!(await sleepAbortable(420))) return;

    for (const msg of chatScript) {
      if (chatAbort) return;
      if (msg.role === "agent") {
        chatTyping.value = true;
        scrollChatToBottom();
        if (!(await sleepAbortable(720))) return;
        chatTyping.value = false;
      } else if (msg.role === "user") {
        if (!(await sleepAbortable(380))) return;
      } else {
        if (!(await sleepAbortable(480))) return;
      }
      visibleMessages.value = [
        ...visibleMessages.value,
        { ...msg, key: `${chatCycle.value}-${seq++}` },
      ];
      await sleep(30);
      scrollChatToBottom();
      if (!(await sleepAbortable(msg.role === "system" ? 900 : 520))) return;
    }

    chatCycle.value += 1;
    if (!(await sleepAbortable(2200))) return;
  }
}

function stopChatLoop() {
  chatAbort = true;
}
const journeyEl = useTemplateRef<HTMLElement>("journey");
const activeStep = ref(0);
const journeyProgress = ref(0);
const isNarrow = ref(false);
function updateNarrow() {
  isNarrow.value = window.matchMedia("(max-width: 900px)").matches;
}

// ScrollTrigger（由 gsap.matchMedia 管理，窄屏不创建 pin）
let journeyMm: gsap.MatchMedia | null = null;
let journeyTrigger: ScrollTrigger | null = null;
let lastProgress = 0;
function applyJourneyProgress(p: number) {
  journeyProgress.value = p;
  // 带方向滞回：只有真正越过阈值才切换，避免停在边界时来回抖动
  const i = Math.min(journeySteps.length - 1, Math.floor(p * journeySteps.length + 0.0001));
  const dir = p - lastProgress;
  if (i !== activeStep.value) {
    if ((i > activeStep.value && dir >= 0) || (i < activeStep.value && dir <= 0)) {
      activeStep.value = i;
    }
  }
  lastProgress = p;
}
function setupJourneyScroll() {
  if (!journeyEl.value) return;
  // 不再 pin：改用 CSS sticky 钉住（见 .journey-sticky），ScrollTrigger 只负责驱动进度。
  // GSAP pin 在 end 到达时会把元素瞬间放回滚动起点，导致滚动完变成白板。
  journeyTrigger = ScrollTrigger.create({
    trigger: journeyEl.value,
    start: "top top",
    end: "bottom bottom",
    scrub: true,
    onUpdate: (self) => applyJourneyProgress(self.progress),
  });
}
function teardownJourneyScroll() {
  journeyTrigger?.kill();
  journeyTrigger = null;
}

let reduceMotion = false;

function prefersReduced() {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/* ── 多 Agent 走马灯（对齐 FeatureCapsulesBoard：GSAP 像素周期 + 两端渐隐） ── */
const agentsMarquee = useTemplateRef<HTMLElement>("agentsMarquee");
const agentsTrack = useTemplateRef<HTMLElement>("agentsTrack");
const agentsStatic = ref(
  typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
);
const agentsSetCopies = computed(() => (agentsStatic.value ? 1 : 3));
let agentsTween: gsap.core.Tween | null = null;

function onMarqueeHover() {
  agentsTween?.pause();
}
function onMarqueeLeave() {
  agentsTween?.play();
}

function teardownAgentsMarquee() {
  agentsTween?.kill();
  agentsTween = null;
  agentsMarquee.value?.removeEventListener("mouseenter", onMarqueeHover);
  agentsMarquee.value?.removeEventListener("mouseleave", onMarqueeLeave);
  if (agentsTrack.value) gsap.set(agentsTrack.value, { clearProps: "transform" });
}

async function setupAgentsMarquee() {
  teardownAgentsMarquee();
  if (!agentsMarquee.value || !agentsTrack.value || prefersReduced()) return;

  await nextTick();
  // 等 logo 解码 + 一帧布局稳定后再量周期（与特性胶囊板同套路）
  const imgs = Array.from(agentsMarquee.value.querySelectorAll<HTMLImageElement>(".agent-logo"));
  await Promise.all(
    imgs.map(
      (img) =>
        new Promise<void>((resolve) => {
          if (img.complete && img.naturalWidth > 0) {
            img.decode?.().then(() => resolve()).catch(() => resolve());
            return;
          }
          img.onload = () => resolve();
          img.onerror = () => resolve();
        })
    )
  );
  await new Promise<void>((r) => requestAnimationFrame(() => r()));

  if (!agentsMarquee.value || !agentsTrack.value) return;
  const set = agentsTrack.value.querySelector<HTMLElement>(".agent-set");
  if (!set) return;
  const period = set.getBoundingClientRect().width;
  if (!period || period <= 0) return;

  const duration = Math.max(22, period / 60);
  gsap.set(agentsTrack.value, { x: 0 });
  agentsTween = gsap.to(agentsTrack.value, {
    x: -period,
    ease: "none",
    duration,
    repeat: -1,
  });
  agentsMarquee.value.addEventListener("mouseenter", onMarqueeHover);
  agentsMarquee.value.addEventListener("mouseleave", onMarqueeLeave);
}

let agentsResizeTimer: ReturnType<typeof setTimeout> | null = null;
function onAgentsResize() {
  if (agentsResizeTimer) clearTimeout(agentsResizeTimer);
  agentsResizeTimer = setTimeout(() => void setupAgentsMarquee(), 140);
}

onMounted(() => {
  reduceMotion = prefersReduced();
  agentsStatic.value = reduceMotion;
  chatAnimLive.value = !reduceMotion;
  updateNarrow();
  chatAbort = false;
  void runChatLoop();
  gsap.registerPlugin(ScrollTrigger);
  void setupAgentsMarquee();
  // 窄屏走静态列表，不创建 pin；桌面端由 matchMedia 自动管理生命周期
  journeyMm = gsap.matchMedia();
  journeyMm.add("(min-width: 901px)", () => {
    setupJourneyScroll();
    return () => teardownJourneyScroll();
  });
  window.addEventListener("resize", updateNarrow, { passive: true });
  window.addEventListener("resize", onAgentsResize, { passive: true });
});

onBeforeUnmount(() => {
  stopChatLoop();
  teardownAgentsMarquee();
  if (agentsResizeTimer) clearTimeout(agentsResizeTimer);
  journeyMm?.revert();
  journeyMm = null;
  window.removeEventListener("resize", updateNarrow);
  window.removeEventListener("resize", onAgentsResize);
});
</script>

<template>
  <div ref="pageRoot" class="landing-page">
    <!-- ── Hero：表面出血 + 内容收栏（本需求保持不变） ── -->
    <section id="hero" class="hero" aria-labelledby="hero-brand">
      <div class="hero-atmosphere" aria-hidden="true">
        <div class="hero-mist-glow" />
        <div class="hero-soil" />
      </div>

      <div class="hero-plane">
        <div class="site-rail hero-rail">
          <div class="hero-copy site-rise-in">
            <p id="hero-brand" class="hero-brand">
              <span class="hero-brand-name">春笋</span>
            </p>

            <h1 class="hero-title">竹林细雨落，春笋破土出</h1>
            <p class="hero-subtitle">
              灵感如细雨落下，春笋破土，无限生长。
            </p>

            <div class="hero-actions">
              <a href="/console/auth/register" class="site-btn site-btn-primary site-btn-lg" @click.prevent="goToRegister">
                开始生长
              </a>
            </div>
          </div>

          <div class="hero-visual" aria-hidden="true">
            <HeroBambooScene />
          </div>
        </div>
      </div>
    </section>

    <!-- ── 多 Agent 支持（全屏出血 · 对齐特性胶囊板滚动方案） ── -->
    <section id="agents" class="agents-band" aria-label="已支持的 Agent">
      <div
        class="agents-marquee-bleed"
        data-reveal
        :class="{ 'is-static': agentsStatic }"
      >
        <div ref="agentsMarquee" class="agents-marquee">
          <div ref="agentsTrack" class="agents-track">
            <div
              v-for="copy in agentsSetCopies"
              :key="'set-' + copy"
              class="agent-set"
              :aria-hidden="copy > 1 ? 'true' : undefined"
            >
              <span v-for="(a, i) in supportedAgents" :key="copy + '-' + i" class="agent-item">
                <img class="agent-logo" :src="a.logo" :alt="copy === 1 ? a.name + ' logo' : ''" />
                <span class="agent-name">{{ a.name }}</span>
              </span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- ── 运行证明：Agent Chat 自主交付 ── -->
    <section id="console" class="console" aria-labelledby="console-title">
      <div class="site-rail console-rail" data-reveal>
        <header class="console-head">
          <p class="eyebrow">开始生长</p>
          <h2 id="console-title" class="section-title">一条斜线命令，自主推进</h2>
          <p class="section-lead">
            Agent 连续决策与验收，直到场景全绿。
          </p>
        </header>

        <div
          class="agent-chat"
          aria-label="春笋自主交付 Agent 会话演示"
          :data-chat-cycle="chatCycle"
        >
          <div class="chat-bar">
            <span class="chat-bar-dot" aria-hidden="true" />
            <div class="chat-bar-meta">
              <span class="chat-bar-title">Agent · 自主推进</span>
              <span class="chat-bar-id">JD1XwcSARhGf</span>
            </div>
            <span class="chat-bar-status" :class="{ live: chatAnimLive }">
              {{ chatAnimLive ? "演示中" : "预览" }}
            </span>
          </div>

          <div ref="chatBody" class="chat-body">
            <div
              v-for="m in visibleMessages"
              :key="m.key"
              :class="['chat-row', `is-${m.role}`]"
            >
              <div v-if="m.role === 'agent'" class="chat-bubble agent">
                <span v-if="m.step" class="chat-step">{{ m.step }}</span>
                <p class="chat-text">{{ m.text }}</p>
              </div>
              <div v-else-if="m.role === 'user'" class="chat-bubble user">
                <p class="chat-text">{{ m.text }}</p>
              </div>
              <div v-else class="chat-system" role="status">
                <Check class="chat-system-icon" :size="14" aria-hidden="true" />
                <span>{{ m.text }}</span>
              </div>
            </div>

            <div v-if="chatTyping" class="chat-row is-agent" aria-hidden="true">
              <div class="chat-bubble agent is-typing">
                <span class="chat-typing-dot" />
                <span class="chat-typing-dot" />
                <span class="chat-typing-dot" />
              </div>
            </div>
          </div>

          <div class="chat-composer" aria-hidden="true">
            <span class="chat-composer-placeholder">输入 /chunsun 启动下一条需求…</span>
            <CornerDownLeft class="chat-composer-send" :size="16" aria-hidden="true" />
          </div>
        </div>
      </div>
    </section>

    <!-- ── 春笋能力 · 特性胶囊 ── -->
    <section id="bento" class="bento" aria-labelledby="bento-title">
      <div class="site-rail" data-reveal>
        <header class="bento-head">
          <p class="eyebrow">持续生长</p>
          <h2 id="bento-title" class="section-title">春笋能力</h2>
          <p class="section-lead">
            接入、记忆、验收、密钥、权限、缺陷闭环……春笋也在持续生长，不止于此。
          </p>
        </header>
      </div>
      <div class="bento-marquee-bleed" data-reveal>
        <FeatureCapsulesBoard />
      </div>
    </section>

    <!-- ── 一条需求的旅程 · 滚动钉住叙事 ── -->
    <section id="journey" ref="journey" class="journey" aria-labelledby="journey-title">
      <div class="journey-sticky">
        <div class="journey-pin">
          <div class="site-rail journey-rail">
            <div class="journey-aside">
              <header class="journey-head">
                <p class="journey-eyebrow">跨会话和用户统一生命周期</p>
                <h2 id="journey-title" class="section-title journey-title">一条需求的旅程</h2>
              </header>
              <nav class="journey-nav" aria-label="需求旅程步骤">
                <ol class="journey-steps">
                  <li
                    v-for="(s, i) in journeySteps"
                    :key="s.id"
                    :class="['j-step', { active: activeStep === i, past: activeStep > i }]"
                    :data-phase="s.id"
                  >
                    <span class="j-step-node" aria-hidden="true">
                      <span class="j-step-core" />
                    </span>
                    <span class="j-step-title">{{ s.t }}</span>
                  </li>
                </ol>
                <div class="journey-track" aria-hidden="true">
                  <span
                    v-for="n in 3"
                    :key="'knuckle-' + n"
                    class="journey-knuckle"
                    :style="{ top: (n * 25) + '%' }"
                  />
                  <span class="journey-fill" :style="{ height: journeyProgress * 100 + '%' }" />
                </div>
              </nav>
            </div>

            <div v-if="!isNarrow" class="journey-stage">
              <div
                v-for="(s, i) in journeySteps"
                :key="s.id"
                class="j-panel"
                :class="{ active: activeStep === i }"
                :aria-hidden="activeStep !== i"
              >
                <JourneyProductMock :phase="s.id" :mock="s.mock" />
                <h3 class="j-panel-title">{{ s.t }}</h3>
                <p class="j-panel-body">{{ s.b }}</p>
              </div>
            </div>

            <div v-else class="journey-stage journey-stage--list">
              <div
                v-for="s in journeySteps"
                :key="s.id"
                class="j-panel"
              >
                <JourneyProductMock :phase="s.id" :mock="s.mock" />
                <h3 class="j-panel-title">{{ s.t }}</h3>
                <p class="j-panel-body">{{ s.b }}</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- ── 底部 CTA ── -->
    <section id="cta" class="cta" aria-labelledby="cta-title">
      <div class="site-rail cta-rail" data-reveal>
        <BrandMark size="3rem" />
        <h2 id="cta-title" class="cta-title">让下一个需求破土而出</h2>
        <p class="cta-body">录入需求，用 /chunsun 启动，交给平台收口。</p>
        <a href="/console/auth/register" class="site-btn site-btn-primary site-btn-lg" @click.prevent="goToRegister">
          开始生长
        </a>
      </div>
    </section>

    <!-- ── 页脚 ── -->
    <footer class="landing-footer">
      <div class="site-rail footer-bar">
        <div class="footer-logo">
          <BrandMark size="1.25rem" />
          <span class="footer-logo-text">春笋</span>
          <span class="footer-version">
            v{{ appVersion }}
          </span>
        </div>
        <div class="footer-meta">
          <a
            class="footer-link"
            :href="GITHUB_URL"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub
          </a>
          <a
            class="footer-link"
            :href="LICENSE_URL"
            target="_blank"
            rel="noopener noreferrer"
          >
            MIT License
          </a>
          <a
            class="footer-link"
            href="https://mengqinghe.com/"
            target="_blank"
            rel="noopener noreferrer"
          >
            孟庆贺
          </a>
          <span class="footer-copy">© 2026 春笋 · Chunsun</span>
        </div>
      </div>
    </footer>
  </div>
</template>

<style scoped>
/* ── Hero（保持原样） ───────────────────────── */
.hero {
  position: relative;
  min-height: 100vh;
  min-height: 100dvh;
  display: flex;
  align-items: stretch;
  overflow: clip;
  background:
    linear-gradient(
      165deg,
      #dfeae2 0%,
      var(--chunsun-mist) 38%,
      var(--chunsun-fog) 72%,
      #e3e7dc 100%
    );
}

.hero-atmosphere {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.hero-mist-glow {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse 80% 50% at 20% 0%, color-mix(in srgb, var(--chunsun-rain) 14%, transparent), transparent 55%);
}

.hero-soil {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  height: 28%;
  background:
    linear-gradient(
      to top,
      color-mix(in srgb, var(--chunsun-soil) 22%, transparent) 0%,
      transparent 100%
    );
}

.hero-plane {
  position: relative;
  z-index: 1;
  flex: 1;
  width: 100%;
  display: flex;
  align-items: center;
  padding-block: clamp(6.5rem, 12vh, 8rem) clamp(2.5rem, 6vh, 4rem);
}

.hero-rail {
  position: relative;
  z-index: 2;
  width: 100%;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 0.95fr);
  gap: clamp(1.5rem, 4vw, 3rem);
  align-items: center;
}

.hero-copy {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  max-width: 34rem;
}

.hero-brand {
  display: flex;
  align-items: center;
  gap: 0.85rem;
  margin: 0;
}

.hero-brand-name {
  font-size: clamp(2.75rem, 7vw, 4.75rem);
  font-weight: 800;
  letter-spacing: -0.04em;
  line-height: 1;
  color: var(--chunsun-ink);
}

.hero-title {
  margin: 0;
  font-size: clamp(1.75rem, 3.6vw, 2.75rem);
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
  color: var(--chunsun-node);
}

.hero-subtitle {
  margin: 0;
  font-size: clamp(1.05rem, 1.6vw, 1.2rem);
  line-height: 1.65;
  color: var(--chunsun-ink-muted);
  max-width: 28rem;
}

.hero-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  margin-top: 0.5rem;
}

.hero-visual {
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  min-height: min(62vh, 560px);
  pointer-events: none;
}

/* ── Shared ─────────────────────────────────── */
.section-title {
  margin: 0;
  font-size: clamp(1.55rem, 3vw, 2.1rem);
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.25;
  color: var(--chunsun-ink);
}

.section-lead {
  margin: 0.85rem 0 0;
  max-width: 38rem;
  font-size: 1.05rem;
  line-height: 1.65;
  color: var(--chunsun-ink-muted);
}

.eyebrow {
  margin: 0 0 0.75rem;
  font-size: 0.78rem;
  font-weight: 700;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  color: var(--chunsun-shoot);
}

code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.92em;
  padding: 0.1em 0.35em;
  border-radius: 0.3rem;
  background: color-mix(in srgb, var(--chunsun-shoot) 12%, transparent);
  color: var(--chunsun-node);
}

/* ── Agent Chat 自主交付演示 ────────────────── */
.console {
  padding-block: clamp(3.5rem, 9vh, 5.5rem) clamp(3rem, 8vh, 5rem);
  background:
    linear-gradient(180deg, #e3e7dc 0%, var(--chunsun-mist) 30%, var(--chunsun-mist) 100%);
}

.console-rail {
  display: grid;
  gap: clamp(1.75rem, 4vw, 2.5rem);
}

.console-head {
  max-width: 44rem;
}

.agent-chat {
  display: flex;
  flex-direction: column;
  height: clamp(26rem, 52vh, 30rem);
  border-radius: 1rem;
  overflow: hidden;
  background: var(--chunsun-fog);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 16%, transparent);
  box-shadow: 0 22px 50px -30px color-mix(in srgb, var(--chunsun-ink) 40%, transparent);
}

.chat-bar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-shrink: 0;
  padding: 0.85rem 1.1rem;
  background: color-mix(in srgb, var(--chunsun-mist) 70%, var(--chunsun-fog));
  border-bottom: 1px solid color-mix(in srgb, var(--chunsun-shoot) 12%, transparent);
}

.chat-bar-dot {
  width: 0.65rem;
  height: 0.65rem;
  border-radius: 50%;
  background: var(--chunsun-shoot);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--chunsun-shoot) 16%, transparent);
  flex-shrink: 0;
}

.chat-bar-meta {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-width: 0;
  flex: 1;
}

.chat-bar-title {
  font-size: 0.92rem;
  font-weight: 700;
  color: var(--chunsun-ink);
}

.chat-bar-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.72rem;
  color: var(--chunsun-ink-muted);
}

.chat-bar-status {
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--chunsun-ink-muted);
  padding: 0.28rem 0.55rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--chunsun-shoot) 10%, transparent);
}

.chat-bar-status.live {
  color: var(--chunsun-shoot);
}

.chat-body {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
  flex: 1 1 auto;
  min-height: 0;
  padding: 1.15rem 1.2rem 1.25rem;
  overflow-y: auto;
  overscroll-behavior: contain;
  scroll-behavior: smooth;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--chunsun-mist) 55%, white) 0%, var(--chunsun-mist) 100%);
}

.chat-row {
  display: flex;
  width: 100%;
  animation: chat-in 0.35s ease both;
}

.chat-row.is-user {
  justify-content: flex-end;
}

.chat-row.is-agent {
  justify-content: flex-start;
}

.chat-row.is-system {
  justify-content: center;
}

.chat-bubble {
  max-width: min(34rem, 88%);
  padding: 0.75rem 0.95rem;
  border-radius: 1rem;
  line-height: 1.55;
}

.chat-bubble.user {
  background: var(--chunsun-shoot);
  color: #f7faf5;
  border-bottom-right-radius: 0.35rem;
}

.chat-bubble.agent {
  background: var(--chunsun-fog);
  color: var(--chunsun-ink);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 14%, transparent);
  border-bottom-left-radius: 0.35rem;
  display: grid;
  gap: 0.4rem;
}

.chat-bubble.is-typing {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.85rem 1rem;
  min-width: 3.2rem;
}

.chat-step {
  display: inline-flex;
  align-self: start;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--chunsun-shoot);
  background: color-mix(in srgb, var(--chunsun-shoot) 12%, transparent);
  padding: 0.15rem 0.45rem;
  border-radius: 0.35rem;
}

.chat-text {
  margin: 0;
  font-size: 0.95rem;
}

.chat-bubble.user .chat-text {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.9rem;
  font-weight: 600;
}

.chat-system {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.55rem 0.9rem;
  border-radius: 999px;
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--chunsun-node);
  background: color-mix(in srgb, var(--chunsun-tip) 18%, var(--chunsun-fog));
  border: 1px solid color-mix(in srgb, var(--chunsun-tip) 35%, transparent);
}

.chat-system-icon {
  color: var(--chunsun-tip);
}

.chat-typing-dot {
  width: 0.35rem;
  height: 0.35rem;
  border-radius: 50%;
  background: color-mix(in srgb, var(--chunsun-ink) 35%, transparent);
  animation: chat-typing 1.1s ease-in-out infinite;
}

.chat-typing-dot:nth-child(2) {
  animation-delay: 0.15s;
}

.chat-typing-dot:nth-child(3) {
  animation-delay: 0.3s;
}

.chat-composer {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-shrink: 0;
  padding: 0.75rem 1rem;
  border-top: 1px solid color-mix(in srgb, var(--chunsun-shoot) 12%, transparent);
  background: var(--chunsun-fog);
}

.chat-composer-placeholder {
  flex: 1;
  font-size: 0.88rem;
  color: var(--chunsun-ink-muted);
  padding: 0.55rem 0.75rem;
  border-radius: 0.65rem;
  background: color-mix(in srgb, var(--chunsun-mist) 80%, transparent);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 10%, transparent);
}

.chat-composer-send {
  width: 2rem;
  height: 2rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 0.55rem;
  font-size: 0.95rem;
  color: var(--chunsun-fog);
  background: color-mix(in srgb, var(--chunsun-shoot) 70%, var(--chunsun-ink));
}

@keyframes chat-in {
  from {
    opacity: 0;
    transform: translateY(0.45rem);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes chat-typing {
  0%,
  80%,
  100% {
    opacity: 0.35;
    transform: translateY(0);
  }
  40% {
    opacity: 1;
    transform: translateY(-0.15rem);
  }
}

/* ── 春笋能力 · 特性胶囊区 ──────────────────── */
.bento {
  padding-block: clamp(4rem, 10vh, 6.5rem);
  background: var(--chunsun-mist);
  overflow-x: clip;
}

.bento-head {
  max-width: 44rem;
}

.bento-marquee-bleed {
  margin-top: clamp(1.75rem, 4vw, 2.5rem);
  /* 相对 site-rail 出血到视口两侧，走马灯两端渐隐更干净 */
  width: 100vw;
  margin-inline: calc(50% - 50vw);
  padding-inline: clamp(0.5rem, 2vw, 1rem);
}

/* ── 滚动钉住叙事 ───────────────────────────── */
.journey {
  position: relative;
  height: 280vh;
  background: var(--chunsun-fog);
}

/* sticky 钉住：section 内滚动期间粘在视口顶部，结束后平滑随 section 滚出（无 GSAP pin 的白板跳变） */
.journey-sticky {
  position: sticky;
  top: 0;
  height: 100vh;
  display: flex;
  align-items: center;
  width: 100%;
  overflow: clip;
}

/* ScrollTrigger 的 pin 目标：天然全宽，避免直接 pin 绝对定位元素导致宽度被压缩 */
.journey-pin {
  width: 100%;
  height: 100vh;
  display: flex;
  align-items: center;
}

.journey-rail {
  width: 100%;
  display: grid;
  grid-template-columns: minmax(12rem, 0.72fr) minmax(0, 1.28fr);
  gap: clamp(2rem, 5vw, 4.5rem);
  align-items: center;
}

.journey-aside {
  display: grid;
  gap: clamp(1.75rem, 3.5vh, 2.75rem);
  align-content: center;
  max-width: 18.5rem;
}

.journey-head {
  display: grid;
  gap: 0.65rem;
}

.journey-eyebrow {
  margin: 0;
  max-width: 12.5em;
  font-size: 0.8rem;
  font-weight: 600;
  line-height: 1.45;
  letter-spacing: 0.02em;
  color: var(--chunsun-shoot);
}

.journey-title {
  max-width: 8.5em;
}

.journey-nav {
  position: relative;
  padding-left: 0.1rem;
}

.journey-steps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.35rem;
}

.j-step {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 1.6rem 1fr;
  align-items: center;
  gap: 0.95rem;
  min-height: 2.85rem;
  padding-block: 0.35rem;
  color: var(--chunsun-ink-muted);
  opacity: 0.48;
  transition: color 0.3s ease, opacity 0.3s ease;
}

.j-step-node {
  width: 1.15rem;
  height: 1.15rem;
  margin-inline: 0.2rem;
  border-radius: 999px;
  display: grid;
  place-items: center;
  background: var(--chunsun-fog);
  border: 2px solid color-mix(in srgb, var(--chunsun-shoot) 34%, transparent);
  box-shadow: 0 0 0 0 transparent;
  transition:
    transform 0.3s ease,
    border-color 0.3s ease,
    background 0.3s ease,
    box-shadow 0.3s ease;
}

.j-step-core {
  width: 0.38rem;
  height: 0.38rem;
  border-radius: 999px;
  background: transparent;
  transition: background 0.3s ease, transform 0.3s ease;
}

.j-step-title {
  font-size: 1.06rem;
  font-weight: 600;
  letter-spacing: -0.01em;
  line-height: 1.2;
  transition: font-size 0.25s ease, font-weight 0.2s ease, color 0.3s ease;
}

.j-step.active {
  opacity: 1;
  color: var(--chunsun-ink);
}

.j-step.active .j-step-title {
  font-size: 1.18rem;
  font-weight: 700;
}

.j-step.active .j-step-node {
  transform: scale(1.18);
  border-width: 2.5px;
}

.j-step.active .j-step-core {
  transform: scale(1.15);
}

/* 当前节点内核：与右侧 mock 四态同源色 */
.j-step.active[data-phase="write"] .j-step-node {
  border-color: var(--chunsun-rain);
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--chunsun-rain) 14%, transparent);
}

.j-step.active[data-phase="write"] .j-step-core {
  background: var(--chunsun-rain);
}

.j-step.active[data-phase="run"] .j-step-node {
  border-color: var(--chunsun-color-info-text);
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--chunsun-color-info-text) 14%, transparent);
}

.j-step.active[data-phase="run"] .j-step-core {
  background: var(--chunsun-color-info-text);
}

.j-step.active[data-phase="resume"] .j-step-node {
  border-color: var(--chunsun-soil);
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--chunsun-soil) 14%, transparent);
}

.j-step.active[data-phase="resume"] .j-step-core {
  background: var(--chunsun-soil);
}

.j-step.active[data-phase="done"] .j-step-node {
  border-color: var(--chunsun-shoot);
  background: color-mix(in srgb, var(--chunsun-shoot) 10%, var(--chunsun-fog));
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--chunsun-shoot) 16%, transparent);
}

.j-step.active[data-phase="done"] .j-step-core {
  background: var(--chunsun-shoot);
}

.j-step.past {
  opacity: 0.86;
  color: color-mix(in srgb, var(--chunsun-ink) 58%, var(--chunsun-ink-muted));
}

.j-step.past .j-step-node {
  background: color-mix(in srgb, var(--chunsun-shoot) 55%, var(--chunsun-fog));
  border-color: color-mix(in srgb, var(--chunsun-shoot) 70%, transparent);
}

.j-step.past .j-step-core {
  background: var(--chunsun-node);
}

.journey-track {
  position: absolute;
  left: 0.72rem;
  top: 1rem;
  bottom: 1rem;
  width: 5px;
  background: color-mix(in srgb, var(--chunsun-shoot) 16%, transparent);
  border-radius: 999px;
  z-index: 0;
  overflow: visible;
}

/* 轻竹节：轨上三段微凸，暗示竹竿而非插画 */
.journey-knuckle {
  position: absolute;
  left: 50%;
  width: 9px;
  height: 7px;
  translate: -50% -50%;
  border-radius: 999px;
  background: color-mix(in srgb, var(--chunsun-shoot) 22%, var(--chunsun-fog));
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 28%, transparent);
  z-index: 0;
  pointer-events: none;
}

.journey-fill {
  position: absolute;
  inset: 0 auto 0 0;
  width: 100%;
  background: var(--chunsun-shoot);
  border-radius: 999px;
  transition: height 0.15s linear;
  z-index: 0;
}

.journey-stage {
  position: relative;
  min-height: 22rem;
  border-radius: 1rem;
  padding: clamp(1rem, 2vw, 1.35rem);
  background: var(--chunsun-mist);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 14%, transparent);
  overflow: hidden;
}

/* 四个面板常驻 DOM，切换仅做交叉淡入淡出（不重挂载 → 无弹跳；绝对定位堆叠 → 无高度突变） */
.j-panel {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
  padding: clamp(1rem, 2vw, 1.35rem);
  opacity: 0;
  transform: translateY(0.6rem);
  transition: opacity 0.35s ease, transform 0.35s ease;
  pointer-events: none;
}

.j-panel.active {
  opacity: 1;
  transform: translateY(0);
}

.j-panel-title {
  margin: 0;
  font-size: 1.15rem;
  font-weight: 700;
  color: var(--chunsun-ink);
}

.j-panel-body {
  margin: 0;
  font-size: 0.95rem;
  line-height: 1.55;
  color: var(--chunsun-ink-muted);
  max-width: 36rem;
}

/* ── 多 Agent 支持（全屏出血 · 对齐特性胶囊板） ── */
.agents-band {
  padding-block: 1.6rem 1.85rem;
  background: linear-gradient(180deg, #e3e7dc 0%, var(--chunsun-mist) 100%);
  overflow-x: clip;
}

.agents-marquee-bleed {
  /* 相对页面出血到视口两侧，与 .bento-marquee-bleed 同方案 */
  width: 100vw;
  margin-inline: calc(50% - 50vw);
  padding-inline: clamp(0.5rem, 2vw, 1rem);
}

.agents-marquee {
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

.agents-track {
  display: flex;
  align-items: center;
  width: max-content;
  will-change: transform;
}

.agent-set {
  display: flex;
  align-items: center;
  gap: 3rem;
  flex-shrink: 0;
}

.agent-item {
  display: inline-flex;
  align-items: center;
  gap: 0.6rem;
  white-space: nowrap;
}

/* 末项右侧补同等 gap，使一组宽度自包含，无缝衔接无断层 */
.agent-set .agent-item:last-child {
  margin-inline-end: 3rem;
}

.agent-logo {
  height: 1.9rem;
  width: auto;
  object-fit: contain;
}

.agent-name {
  font-size: 1.05rem;
  font-weight: 700;
  letter-spacing: -0.01em;
  color: color-mix(in srgb, var(--chunsun-node) 60%, var(--chunsun-rain));
}

/* 降动效：静态居中平铺，不做走马灯 */
.agents-marquee-bleed.is-static .agents-marquee {
  overflow: visible;
  -webkit-mask-image: none;
  mask-image: none;
}

.agents-marquee-bleed.is-static .agents-track {
  width: auto;
  flex-wrap: wrap;
  justify-content: center;
  row-gap: 0.75rem;
}

.agents-marquee-bleed.is-static .agent-set {
  flex-wrap: wrap;
  justify-content: center;
  width: 100%;
  gap: 1.5rem 2.5rem;
}

.agents-marquee-bleed.is-static .agent-set .agent-item:last-child {
  margin-inline-end: 0;
}

/* ── CTA ────────────────────────────────────── */
.cta {
  padding-block: clamp(4.5rem, 12vh, 7rem);
  background:
    linear-gradient(165deg, #dfeae2 0%, var(--chunsun-mist) 45%, #e3e7dc 100%);
}

.cta-rail {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1rem;
  max-width: 36rem;
}

.cta-title {
  margin: 0.25rem 0 0;
  font-size: clamp(1.55rem, 3vw, 2.15rem);
  font-weight: 700;
  letter-spacing: -0.03em;
  color: var(--chunsun-ink);
}

.cta-body {
  margin: 0 0 0.5rem;
  font-size: 1.05rem;
  line-height: 1.65;
  color: var(--chunsun-ink-muted);
}

/* ── Footer ─────────────────────────────────── */
.landing-footer {
  padding: 1.5rem 0;
  background: var(--chunsun-mist);
  border-top: 1px solid color-mix(in srgb, var(--chunsun-shoot) 14%, transparent);
}

.footer-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.footer-logo {
  display: flex;
  align-items: center;
  gap: 0.45rem;
}

.footer-logo-text {
  font-weight: 700;
  font-size: 0.95rem;
  color: var(--chunsun-ink);
}

.footer-version {
  display: inline-flex;
  align-items: center;
  gap: 0.28rem;
  margin-left: 0.15rem;
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
  color: var(--chunsun-ink-muted);
}

.footer-copy {
  font-size: 0.8rem;
  color: var(--chunsun-ink-muted);
}

.footer-meta {
  display: flex;
  align-items: center;
  gap: 0.85rem;
  flex-wrap: wrap;
}

.footer-link {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.8rem;
  color: var(--chunsun-node);
  text-decoration: none;
  text-underline-offset: 0.22em;
  text-decoration-thickness: 1.25px;
  transition: color 0.18s ease, text-decoration-color 0.18s ease;
}

.footer-link:hover {
  color: var(--chunsun-shoot);
  text-decoration-line: underline;
  text-decoration-style: wavy;
  text-decoration-color: color-mix(in srgb, var(--chunsun-shoot) 70%, transparent);
}

/* ── Scroll reveal（useLandingReveal 驱动） ──── */
@keyframes landing-reveal-rise {
  from {
    opacity: 0;
    transform: translateY(1.35rem);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.landing-reveal-io [data-reveal] {
  opacity: 0;
  transform: translateY(1.35rem);
  transition:
    opacity 0.7s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.7s cubic-bezier(0.22, 1, 0.36, 1);
}

.landing-reveal-io [data-reveal].is-revealed {
  opacity: 1;
  transform: translateY(0);
}

@media (prefers-reduced-motion: no-preference) {
  @supports ((animation-timeline: view()) and (animation-range: entry)) {
    .landing-reveal-native [data-reveal] {
      animation: landing-reveal-rise auto linear both;
      animation-timeline: view();
      animation-range: entry 0% entry 35%;
    }
  }
}

.landing-reveal-reduced [data-reveal] {
  opacity: 1;
  transform: none;
}

@media (prefers-reduced-motion: reduce) {
  .landing-reveal-io [data-reveal],
  .landing-reveal-native [data-reveal] {
    animation: none !important;
    opacity: 1 !important;
    transform: none !important;
    transition: none !important;
  }
  .chat-row {
    animation: none !important;
  }
  .chat-typing-dot {
    animation: none !important;
  }
  .journey-fill {
    transition: none !important;
  }
  .j-panel {
    transition: none !important;
  }
  .j-step-node,
  .j-step-core,
  .j-step-title {
    transition: none !important;
  }
  .j-step.active .j-step-node,
  .j-step.active .j-step-core {
    transform: none !important;
  }
  .j-step.active .j-step-title {
    font-size: 1.18rem;
  }
}

/* ── Responsive ─────────────────────────────── */
@media (max-width: 900px) {
  .hero-plane {
    align-items: flex-start;
    padding-top: 5.5rem;
  }
  .hero-rail {
    grid-template-columns: 1fr;
    gap: 1.25rem;
  }
  .hero-visual {
    order: -1;
    min-height: min(42vh, 320px);
    justify-content: center;
  }

  .journey {
    height: auto;
  }
  .journey-sticky {
    position: static;
    height: auto;
    padding-block: clamp(3rem, 8vh, 5rem);
  }
  .journey-pin {
    height: auto;
  }
  .journey-rail {
    grid-template-columns: 1fr;
    gap: 2rem;
  }
  .journey-aside {
    max-width: none;
  }
  .journey-eyebrow {
    max-width: none;
  }
  .journey-track {
    display: none;
  }
  .journey-stage {
    min-height: auto;
  }
  .journey-stage--list {
    overflow: visible;
  }
  .j-panel {
    position: static;
    padding: 0;
    opacity: 1;
    transform: none;
    transition: none;
    pointer-events: auto;
    margin-bottom: 1.25rem;
  }
  .journey-stage--list .j-panel:last-child {
    margin-bottom: 0;
  }
}

@media (max-width: 640px) {
  .hero-brand-name {
    font-size: 2.5rem;
  }
}
</style>
