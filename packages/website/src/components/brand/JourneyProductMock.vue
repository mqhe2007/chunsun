<script setup lang="ts">
export type JourneyPhase = "write" | "run" | "resume" | "done";

export type JourneyMock = {
  statusLabel: string;
  stats: { k: string; v: string; warn?: boolean }[];
  runTitle: string;
  runStatus: string;
  runPaused?: string;
  runReason?: string;
  steps: { kind: string; summary: string; done?: boolean; active?: boolean }[];
  emptyRun?: string;
  scenarios: { title: string; status: "pending" | "passing" | "failing" | "blocked" }[];
  decision?: string;
  footer?: string;
};

defineProps<{
  phase: JourneyPhase;
  mock: JourneyMock;
}>();

const scenarioStatusLabel: Record<JourneyMock["scenarios"][number]["status"], string> = {
  pending: "待验收",
  passing: "通过",
  failing: "失败",
  blocked: "受阻",
};
</script>

<template>
  <div class="j-mock" :data-phase="phase" aria-hidden="true">
    <header class="j-mock-chrome">
      <span class="j-mock-crumb">需求</span>
      <span class="j-mock-id">JD1XwcSARhGf</span>
      <span class="j-mock-pill" :data-phase="phase">{{ mock.statusLabel }}</span>
    </header>
    <div class="j-mock-stats">
      <div
        v-for="st in mock.stats"
        :key="st.k"
        class="j-mock-stat"
        :class="{ warn: st.warn }"
      >
        <span class="j-mock-stat-v">{{ st.v }}</span>
        <span class="j-mock-stat-k">{{ st.k }}</span>
      </div>
    </div>
    <div class="j-mock-grid">
      <div class="j-mock-col">
        <p class="j-mock-label">{{ mock.runTitle }}</p>
        <span v-if="mock.runStatus" class="j-mock-run-status" :data-phase="phase">
          {{ mock.runStatus }}
        </span>
        <p v-if="mock.runPaused" class="j-mock-pause">{{ mock.runPaused }}</p>
        <p v-if="mock.emptyRun" class="j-mock-empty">{{ mock.emptyRun }}</p>
        <ul v-else class="j-mock-steps">
          <li
            v-for="(st, si) in mock.steps"
            :key="si"
            :class="{ done: st.done, active: st.active }"
          >
            <span class="j-mock-kind">{{ st.kind }}</span>
            <span class="j-mock-sum">{{ st.summary }}</span>
          </li>
        </ul>
      </div>
      <div class="j-mock-col">
        <p class="j-mock-label">验收场景</p>
        <ul class="j-mock-scenarios">
          <li
            v-for="(sc, sci) in mock.scenarios"
            :key="sci"
            :data-status="sc.status"
          >
            <span class="j-mock-sc-title">{{ sc.title }}</span>
            <span class="j-mock-sc-status">{{ scenarioStatusLabel[sc.status] }}</span>
          </li>
        </ul>
        <div v-if="mock.decision" class="j-mock-memory">
          <p class="j-mock-label">工作记忆 · 未决决策</p>
          <p class="j-mock-decision">{{ mock.decision }}</p>
        </div>
      </div>
    </div>
    <p v-if="mock.footer" class="j-mock-footer">{{ mock.footer }}</p>
  </div>
</template>

<style scoped>
.j-mock {
  display: grid;
  gap: 0.75rem;
  padding: 0.85rem 0.95rem;
  border-radius: 0.75rem;
  background: var(--chunsun-fog);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 16%, transparent);
  font-size: 0.78rem;
  line-height: 1.4;
  color: var(--chunsun-ink);
}

.j-mock-chrome {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.j-mock-crumb {
  font-weight: 700;
  color: var(--chunsun-ink-muted);
}

.j-mock-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.72rem;
  color: var(--chunsun-rain);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.j-mock-pill {
  margin-left: auto;
  flex-shrink: 0;
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.68rem;
  font-weight: 700;
  background: color-mix(in srgb, var(--chunsun-rain) 14%, transparent);
  color: var(--chunsun-ink-muted);
}

.j-mock-pill[data-phase="run"] {
  background: var(--chunsun-color-info-bg);
  color: var(--chunsun-color-info-text);
}

.j-mock-pill[data-phase="resume"] {
  background: color-mix(in srgb, var(--chunsun-soil) 18%, transparent);
  color: var(--chunsun-soil);
}

.j-mock-pill[data-phase="done"] {
  background: color-mix(in srgb, var(--chunsun-shoot) 18%, transparent);
  color: var(--chunsun-node);
}

.j-mock-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.4rem;
}

.j-mock-stat {
  display: grid;
  gap: 0.1rem;
  padding: 0.4rem 0.45rem;
  border-radius: 0.45rem;
  background: var(--chunsun-mist);
}

.j-mock-stat.warn .j-mock-stat-v {
  color: var(--chunsun-soil);
}

.j-mock-stat-v {
  font-weight: 800;
  font-size: 0.9rem;
  letter-spacing: -0.02em;
}

.j-mock-stat-k {
  font-size: 0.62rem;
  color: var(--chunsun-ink-muted);
}

.j-mock-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.65rem;
}

.j-mock-col {
  display: grid;
  gap: 0.4rem;
  align-content: start;
  min-width: 0;
}

.j-mock-label {
  margin: 0;
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--chunsun-ink-muted);
}

.j-mock-run-status {
  justify-self: start;
  font-size: 0.68rem;
  font-weight: 700;
  padding: 0.1rem 0.4rem;
  border-radius: 0.3rem;
  background: color-mix(in srgb, var(--chunsun-rain) 12%, transparent);
  color: var(--chunsun-ink-muted);
}

.j-mock-run-status[data-phase="run"] {
  background: var(--chunsun-color-info-bg);
  color: var(--chunsun-color-info-text);
}

.j-mock-run-status[data-phase="resume"] {
  background: color-mix(in srgb, var(--chunsun-soil) 14%, transparent);
  color: var(--chunsun-soil);
}

.j-mock-run-status[data-phase="done"] {
  background: color-mix(in srgb, var(--chunsun-shoot) 14%, transparent);
  color: var(--chunsun-node);
}

.j-mock-pause,
.j-mock-empty,
.j-mock-decision {
  margin: 0;
  color: var(--chunsun-ink-muted);
}

.j-mock-pause {
  color: var(--chunsun-soil);
}

.j-mock-steps,
.j-mock-scenarios {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.3rem;
}

.j-mock-steps li,
.j-mock-scenarios li {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  padding: 0.28rem 0.35rem;
  border-radius: 0.35rem;
  background: var(--chunsun-mist);
  min-width: 0;
}

.j-mock-steps li.done {
  opacity: 0.72;
}

.j-mock-steps li.active {
  outline: 1px solid color-mix(in srgb, var(--chunsun-shoot) 35%, transparent);
  background: color-mix(in srgb, var(--chunsun-shoot) 8%, var(--chunsun-mist));
}

.j-mock-kind {
  flex-shrink: 0;
  font-size: 0.62rem;
  font-weight: 800;
  color: var(--chunsun-node);
}

.j-mock-sum,
.j-mock-sc-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.j-mock-sc-status {
  margin-left: auto;
  flex-shrink: 0;
  font-size: 0.62rem;
  font-weight: 700;
}

.j-mock-scenarios li[data-status="pending"] .j-mock-sc-status {
  color: var(--chunsun-rain);
}

.j-mock-scenarios li[data-status="passing"] .j-mock-sc-status {
  color: var(--chunsun-node);
}

.j-mock-scenarios li[data-status="failing"] .j-mock-sc-status,
.j-mock-scenarios li[data-status="blocked"] .j-mock-sc-status {
  color: var(--chunsun-soil);
}

.j-mock-memory {
  display: grid;
  gap: 0.3rem;
  padding: 0.45rem;
  border-radius: 0.4rem;
  background: color-mix(in srgb, var(--chunsun-soil) 8%, var(--chunsun-mist));
  border: 1px solid color-mix(in srgb, var(--chunsun-soil) 22%, transparent);
}

.j-mock-footer {
  margin: 0;
  text-align: center;
  font-weight: 700;
  color: var(--chunsun-node);
  padding: 0.35rem;
  border-radius: 0.4rem;
  background: color-mix(in srgb, var(--chunsun-shoot) 10%, transparent);
}

@media (max-width: 640px) {
  .j-mock-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .j-mock-grid {
    grid-template-columns: 1fr;
  }
}
</style>
