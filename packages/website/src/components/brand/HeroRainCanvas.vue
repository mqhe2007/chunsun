<script setup lang="ts">
/**
 * 英雄区细雨：Canvas 粒子斜落，轻量「竹林细雨」氛围。
 * 用原生 rAF 而非 GSAP——雨滴是持续粒子场，不需要时间轴编排。
 *
 * 调参：改下方 RAIN 对象即可，热更新后刷新页面。
 */
import { onMounted, onUnmounted, ref } from "vue";

/** ── 可调参数（改这里） ─────────────────────────── */
const RAIN = {
  /** 颜色，对应 --chunsun-rain（olive.500） */
  rgb: "92, 122, 134",
  /** 倾斜角度（度），越大越斜 */
  angleDeg: -45,
  /**
   * 密度：每多少 px² 一滴。
   * 越小越密。建议 2200–5000；常见：疏 4800 / 中 3000 / 密 2200
   */
  areaPerDrop: 1000,
  /** 最少 / 最多雨滴数（封顶防卡顿；限定竹林区域后面积更小） */
  minDrops: 48,
  maxDrops: 180,
  /** 近景占比 0–1（越大近景越多、整体更「实」） */
  nearRatio: 0.48,
  /** 近景：[最小值, 最大值] */
  near: {
    len: [10, 26] as const,
    speed: [3.2, 5.8] as const,
    width: [1.1, 1.65] as const,
    alpha: [0.2, 0.44] as const,
  },
  /** 远景：更短更淡更慢 */
  far: {
    len: [6, 16] as const,
    speed: [1.6, 3.5] as const,
    width: [0.7, 1.1] as const,
    alpha: [0.09, 0.22] as const,
  },
} as const;

const root = ref<HTMLCanvasElement | null>(null);

type Drop = {
  x: number;
  y: number;
  len: number;
  speed: number;
  width: number;
  alpha: number;
  layer: 0 | 1;
};

const ANGLE = (RAIN.angleDeg * Math.PI) / 180;
const DX = Math.sin(ANGLE);
const DY = Math.cos(ANGLE);

function randBetween([min, max]: readonly [number, number]) {
  return min + Math.random() * (max - min);
}

let drops: Drop[] = [];
let raf = 0;
let running = false;
let width = 0;
let height = 0;
let dpr = 1;
let ctx: CanvasRenderingContext2D | null = null;
let ro: ResizeObserver | null = null;
let io: IntersectionObserver | null = null;
let mq: MediaQueryList | null = null;
let visible = true;
let reduced = false;

function prefersReducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function dropCount(w: number, h: number) {
  return Math.min(
    RAIN.maxDrops,
    Math.max(RAIN.minDrops, Math.round((w * h) / RAIN.areaPerDrop)),
  );
}

type SpawnMode = "fill" | "edge";

/**
 * 斜雨必须从「顶边 + 来向侧边」补粒子，否则对角半区会逐渐空掉。
 * angleDeg < 0 → 左下飘，从右侧补；> 0 → 右下飘，从左侧补。
 */
function spawnDrop(w: number, h: number, mode: SpawnMode): Drop {
  const near = Math.random() < RAIN.nearRatio;
  const p = near ? RAIN.near : RAIN.far;
  const len = randBetween(p.len);
  const margin = 40;

  let x: number;
  let y: number;

  if (mode === "fill") {
    x = Math.random() * (w + margin * 2) - margin;
    y = Math.random() * h;
  } else if (Math.random() < 0.55) {
    // 顶边进入
    x = Math.random() * (w + margin * 2) - margin;
    y = -len - Math.random() * margin;
  } else if (DX < 0) {
    // 左侧飘落：从右侧进入
    x = w + margin + Math.random() * margin;
    y = Math.random() * (h + margin) - margin;
  } else {
    // 右侧飘落：从左侧进入
    x = -margin - Math.random() * margin;
    y = Math.random() * (h + margin) - margin;
  }

  return {
    x,
    y,
    len,
    speed: randBetween(p.speed),
    width: randBetween(p.width),
    alpha: randBetween(p.alpha),
    layer: near ? 1 : 0,
  };
}

function outOfBounds(d: Drop, w: number, h: number) {
  const m = 40;
  if (d.y > h + m) return true;
  if (DX < 0) return d.x < -m;
  return d.x > w + m;
}

function rebuild(w: number, h: number) {
  const n = dropCount(w, h);
  drops = Array.from({ length: n }, () => spawnDrop(w, h, "fill"));
}

function resize() {
  const canvas = root.value;
  if (!canvas) return;

  const parent = canvas.parentElement;
  if (!parent) return;

  const rect = parent.getBoundingClientRect();
  width = Math.max(1, Math.floor(rect.width));
  height = Math.max(1, Math.floor(rect.height));
  dpr = Math.min(window.devicePixelRatio || 1, 2);

  canvas.width = Math.floor(width * dpr);
  canvas.height = Math.floor(height * dpr);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  rebuild(width, height);
  if (reduced) drawStatic();
}

function drawDrop(c: CanvasRenderingContext2D, d: Drop, alphaScale = 1) {
  const x2 = d.x + DX * d.len;
  const y2 = d.y + DY * d.len;
  const a = d.alpha * alphaScale;
  const grad = c.createLinearGradient(d.x, d.y, x2, y2);
  grad.addColorStop(0, `rgba(${RAIN.rgb}, 0)`);
  grad.addColorStop(0.35, `rgba(${RAIN.rgb}, ${a})`);
  grad.addColorStop(1, `rgba(${RAIN.rgb}, ${a * 0.25})`);
  c.beginPath();
  c.moveTo(d.x, d.y);
  c.lineTo(x2, y2);
  c.strokeStyle = grad;
  c.lineWidth = d.width;
  c.lineCap = "round";
  c.stroke();
}

function drawStatic() {
  if (!ctx || !width || !height) return;
  ctx.clearRect(0, 0, width, height);
  for (const d of drops) drawDrop(ctx, d, 0.7);
}

function tick() {
  if (!running || !ctx) return;

  ctx.clearRect(0, 0, width, height);

  for (const layer of [0, 1] as const) {
    for (const d of drops) {
      if (d.layer !== layer) continue;

      d.x += DX * d.speed;
      d.y += DY * d.speed;

      if (outOfBounds(d, width, height)) {
        Object.assign(d, spawnDrop(width, height, "edge"));
      }

      drawDrop(ctx, d);
    }
  }

  raf = requestAnimationFrame(tick);
}

function start() {
  if (running || reduced || !visible) return;
  running = true;
  raf = requestAnimationFrame(tick);
}

function stop() {
  running = false;
  if (raf) {
    cancelAnimationFrame(raf);
    raf = 0;
  }
}

function onMqChange() {
  reduced = prefersReducedMotion();
  if (reduced) {
    stop();
    resize();
  } else if (visible) {
    resize();
    start();
  }
}

onMounted(() => {
  const canvas = root.value;
  if (!canvas) return;

  reduced = prefersReducedMotion();
  resize();

  ro = new ResizeObserver(() => {
    resize();
    if (!reduced && visible) start();
  });
  if (canvas.parentElement) ro.observe(canvas.parentElement);

  io = new IntersectionObserver(
    ([entry]) => {
      visible = entry?.isIntersecting ?? false;
      if (visible && !reduced) start();
      else stop();
    },
    { threshold: 0.05 },
  );
  io.observe(canvas);

  mq = window.matchMedia("(prefers-reduced-motion: reduce)");
  mq.addEventListener("change", onMqChange);

  if (!reduced) start();
});

onUnmounted(() => {
  stop();
  ro?.disconnect();
  io?.disconnect();
  mq?.removeEventListener("change", onMqChange);
});
</script>

<template>
  <canvas ref="root" class="hero-rain-canvas" aria-hidden="true" />
</template>

<style scoped>
.hero-rain-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  /* 上下羽化，避免雨丝硬切竹林边界 */
  mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    #000 8%,
    #000 78%,
    transparent 100%
  );
}
</style>
