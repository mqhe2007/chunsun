<script setup lang="ts">
/**
 * 英雄区主视觉：竹林默认可见 + 整丛摆动；近景/远景春笋破土。
 */
import { onMounted, onUnmounted, ref } from "vue";
import { gsap } from "gsap";
import bambooGroveUrl from "@/assets/brand/bamboo-grove.svg";
import {
  LOGO_PART_A,
  LOGO_PART_B,
  LOGO_TRANSFORM,
} from "@chunsun/web-shared/logoMarkPaths";
import HeroRainCanvas from "@/components/brand/HeroRainCanvas.vue";

const root = ref<HTMLElement | null>(null);

/** 与 bamboo-grove.svg 同 viewBox；竹根约 y≈608 */
const GROUND_Y = 605;

type Shoot = {
  id: string;
  x: number;
  baseY: number;
  scale: number;
  rotate: number;
  color: string;
  /** 相对入场起点的错峰（秒） */
  delay: number;
  /** 第一阶段：两形重叠破土时长 */
  riseDur: number;
  /** 第二阶段：上层图形抽出时长 */
  extractDur: number;
  /** 抽出相对破土开始的滞后 */
  extractLag: number;
  opacity?: number;
};

/** 近景笋（竹前）——错峰与时长各不相同 */
const nearShoots: Shoot[] = [
  { id: "n1", x: 120, baseY: GROUND_Y, scale: 0.048, rotate: -8, color: "#5a8a4e", delay: 0.15, riseDur: 0.72, extractDur: 0.88, extractLag: 0.55 },
  { id: "n2", x: 210, baseY: GROUND_Y - 2, scale: 0.062, rotate: 2, color: "#16a34a", delay: 0.85, riseDur: 1.05, extractDur: 1.15, extractLag: 0.8 },
  { id: "n3", x: 320, baseY: GROUND_Y + 1, scale: 0.04, rotate: 10, color: "#6b9458", delay: 1.55, riseDur: 0.9, extractDur: 0.78, extractLag: 0.65 },
  { id: "n4", x: 165, baseY: GROUND_Y + 2, scale: 0.032, rotate: -3, color: "#7a9e62", delay: 2.2, riseDur: 0.65, extractDur: 1.05, extractLag: 0.5 },
];

/** 远景笋（竹后，更小更淡）——更早、更慢、错开 */
const farShoots: Shoot[] = [
  { id: "f1", x: 70, baseY: GROUND_Y - 6, scale: 0.022, rotate: -5, color: "#6a7f62", delay: 0, riseDur: 1.2, extractDur: 1.35, extractLag: 0.9, opacity: 0.45 },
  { id: "f2", x: 380, baseY: GROUND_Y - 4, scale: 0.028, rotate: 6, color: "#5f7a58", delay: 0.55, riseDur: 0.95, extractDur: 1.1, extractLag: 0.7, opacity: 0.4 },
  { id: "f3", x: 430, baseY: GROUND_Y - 8, scale: 0.018, rotate: -3, color: "#71866c", delay: 1.25, riseDur: 1.4, extractDur: 0.95, extractLag: 1.05, opacity: 0.35 },
  { id: "f4", x: 40, baseY: GROUND_Y - 2, scale: 0.016, rotate: 8, color: "#657860", delay: 1.9, riseDur: 0.8, extractDur: 1.25, extractLag: 0.6, opacity: 0.38 },
];

let ctx: gsap.Context | null = null;

function prefersReducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function irregularSway(el: Element, amp: number, durMin: number, durMax: number) {
  gsap.set(el, { transformOrigin: "50% 100%" });
  const tick = () => {
    gsap.to(el, {
      rotation: gsap.utils.random(-amp, amp),
      skewX: gsap.utils.random(-amp * 0.25, amp * 0.25),
      duration: gsap.utils.random(durMin, durMax),
      ease: "sine.inOut",
      onComplete: tick,
    });
  };
  gsap.delayedCall(0.15, tick);
}

function animateShoot(
  el: HTMLElement,
  shoot: Shoot,
  tl: gsap.core.Timeline,
  i: number,
) {
  const group = el.querySelector(`.shoot[data-id="${shoot.id}"]`);
  const body = group?.querySelector(".shoot-body");
  const tip = group?.querySelector(".shoot-tip");
  const breathe = group?.querySelector(".shoot-breathe");
  if (!body || !tip || !breathe) return;

  const peakOpacity = shoot.opacity ?? 1;
  const start = shoot.delay;
  // 上层图形下压量：与笋尺寸成正比，两形叠成一团
  const overlapY = Math.round(shoot.scale * 920);
  const buryY = Math.round(48 + shoot.scale * 400);

  // 初始：埋在土下；tip 额外下压，与 body 重叠
  gsap.set(body, { y: buryY, scaleY: 0.55, opacity: 0, transformOrigin: "50% 100%" });
  gsap.set(tip, {
    y: buryY + overlapY,
    scaleY: 0.55,
    opacity: 0,
    transformOrigin: "50% 100%",
  });

  // 第一阶段：两形重叠，一起破土出现
  tl.to(
    body,
    {
      y: 0,
      scaleY: 1,
      opacity: peakOpacity,
      duration: shoot.riseDur,
      ease: "expo.out",
    },
    start,
  ).to(
    tip,
    {
      y: overlapY,
      scaleY: 1,
      opacity: peakOpacity,
      duration: shoot.riseDur,
      ease: "expo.out",
    },
    start,
  );

  // 第二阶段：上层图形向上抽出，形成最终 logo 构图
  tl.to(
    tip,
    {
      y: 0,
      duration: shoot.extractDur,
      ease: "back.out(1.6)",
    },
    start + shoot.extractLag,
  );

  tl.to(
    breathe,
    {
      y: i % 2 === 0 ? -2 : -3,
      duration: 2.4 + (i % 4) * 0.45,
      ease: "sine.inOut",
      yoyo: true,
      repeat: -1,
    },
    start + shoot.extractLag + shoot.extractDur * 0.5,
  );
}

onMounted(() => {
  const el = root.value;
  if (!el) return;

  ctx = gsap.context(() => {
    const bamboo = el.querySelector(".bamboo-sway");
    if (!bamboo) return;

    if (prefersReducedMotion()) {
      [...farShoots, ...nearShoots].forEach((s) => {
        const g = el.querySelector(`.shoot[data-id="${s.id}"]`);
        if (!g) return;
        gsap.set(g.querySelectorAll(".shoot-body, .shoot-tip"), {
          opacity: s.opacity ?? 1,
          y: 0,
          scaleY: 1,
        });
      });
      return;
    }

    // 竹子默认就在，只摆动；笋初始态在 animateShoot 里按枚设置
    gsap.set(bamboo, { transformOrigin: "50% 100%" });

    const tl = gsap.timeline();

    farShoots.forEach((shoot, i) => {
      animateShoot(el, shoot, tl, i);
    });

    nearShoots.forEach((shoot, i) => {
      animateShoot(el, shoot, tl, i + farShoots.length);
    });

    irregularSway(bamboo, 2.2, 2.4, 4.6);
  }, el);
});

onUnmounted(() => {
  ctx?.revert();
  ctx = null;
});
</script>

<template>
  <div ref="root" class="hero-bamboo" aria-hidden="true">
    <div class="scene">
      <div class="hero-bamboo-rain">
        <HeroRainCanvas />
      </div>
      <!-- 远景笋：竹后 -->
      <svg
        class="shoot-layer shoot-layer--far"
        viewBox="0 0 640 640"
        xmlns="http://www.w3.org/2000/svg"
        preserveAspectRatio="xMidYMid meet"
      >
        <g
          v-for="shoot in farShoots"
          :key="shoot.id"
          class="shoot"
          :data-id="shoot.id"
          :transform="`translate(${shoot.x} ${shoot.baseY})`"
        >
          <g class="shoot-breathe">
            <g class="shoot-body" :style="{ color: shoot.color }">
              <g
                :transform="`rotate(${shoot.rotate}) scale(${shoot.scale}) translate(-650 -2800)`"
              >
                <g :transform="LOGO_TRANSFORM" fill="currentColor">
                  <path :d="LOGO_PART_B" />
                </g>
              </g>
            </g>
            <g class="shoot-tip" :style="{ color: shoot.color }">
              <g
                :transform="`rotate(${shoot.rotate}) scale(${shoot.scale}) translate(-650 -2800)`"
              >
                <g :transform="LOGO_TRANSFORM" fill="currentColor">
                  <path :d="LOGO_PART_A" />
                </g>
              </g>
            </g>
          </g>
        </g>
      </svg>

      <div class="bamboo-sway">
        <img
          class="bamboo-img"
          :src="bambooGroveUrl"
          alt=""
          width="640"
          height="640"
          decoding="async"
        />
      </div>

      <!-- 近景笋：竹前 -->
      <svg
        class="shoot-layer shoot-layer--near"
        viewBox="0 0 640 640"
        xmlns="http://www.w3.org/2000/svg"
        preserveAspectRatio="xMidYMid meet"
      >
        <g
          v-for="shoot in nearShoots"
          :key="shoot.id"
          class="shoot"
          :data-id="shoot.id"
          :transform="`translate(${shoot.x} ${shoot.baseY})`"
        >
          <g class="shoot-breathe">
            <g class="shoot-body" :style="{ color: shoot.color }">
              <g
                :transform="`rotate(${shoot.rotate}) scale(${shoot.scale}) translate(-650 -2800)`"
              >
                <g :transform="LOGO_TRANSFORM" fill="currentColor">
                  <path :d="LOGO_PART_B" />
                </g>
              </g>
            </g>
            <g class="shoot-tip" :style="{ color: shoot.color }">
              <g
                :transform="`rotate(${shoot.rotate}) scale(${shoot.scale}) translate(-650 -2800)`"
              >
                <g :transform="LOGO_TRANSFORM" fill="currentColor">
                  <path :d="LOGO_PART_A" />
                </g>
              </g>
            </g>
          </g>
        </g>
      </svg>
    </div>
  </div>
</template>

<style scoped>
.hero-bamboo {
  position: relative;
  display: flex;
  justify-content: flex-end;
  align-items: flex-end;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--chunsun-node, #15803d);
}

.scene {
  position: relative;
  width: 100%;
  height: 100%;
  max-width: 640px;
  aspect-ratio: 1;
  max-height: 100%;
}

.hero-bamboo-rain {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 3;
  overflow: hidden;
}

.bamboo-sway {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  will-change: transform;
}

.bamboo-img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.shoot-layer {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  overflow: visible;
}

.shoot-layer--far {
  z-index: 0;
}

.shoot-layer--near {
  z-index: 2;
}

@media (max-width: 900px) {
  .hero-bamboo {
    justify-content: center;
  }

  .scene {
    width: min(100%, 360px);
    margin-inline: auto;
  }
}
</style>
