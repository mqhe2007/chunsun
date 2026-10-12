// Console「编辑态依赖回填 / 保存生效」真实 UI 验证（headless Chrome + CDP，零第三方依赖）。
//
// 背景（需求 g7eg0oqNTXSH）：此前编辑表单把 blockedBy 放进 PATCH，但后端不接收该字段，
// 且编辑回填恒为空数组——「创建后改依赖」静默失效。本脚本走真实浏览器路径验证修复：
//   1. 需求：编辑前回填现有依赖 chip → 点 × 移除 → 保存 → （配合 API 断言）落库生效
//   2. 缺陷：编辑前回填现有依赖 chip
//
// 前置（本机 dev）：
//   - 后端：PORT=11112 cargo run --manifest-path packages/backend/Cargo.toml
//   - console dev：corepack pnpm --filter console dev（11114，应用在 /console/ 下）
//   - headless Chrome：
//       "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new \
//         --remote-debugging-port=9333 --user-data-dir=/tmp/chrome-cdp-profile about:blank
//   - 测试数据（项目需先有 Secret Key，否则项目详情被密钥门禁挡回总览）：
//       PID=项目ID；R2=前置需求（被谁阻塞）；R1=目标需求（blockedBy=[R2]）；
//       D1=目标缺陷（blockedBy=[R2]）
//
// 用法：
//   CDP_TOKEN=<JWT> CDP_PID=<PID> CDP_R1=<R1> CDP_R2=<R2> CDP_D1=<D1> \
//     node packages/console/scripts/verify-dep-edit.mjs requirement   # 回填+移除+保存
//   ... 同上 ... node packages/console/scripts/verify-dep-edit.mjs defect        # 回填
//
// 退出码 0 = 全部断言通过。
const DEBUG = process.env.CDP_DEBUG_URL || "http://127.0.0.1:9333";
const ORIGIN = process.env.CDP_CONSOLE_ORIGIN || "http://127.0.0.1:11114/console";
const token = process.env.CDP_TOKEN;
const pid = process.env.CDP_PID;
const mode = process.argv[2] || "requirement";

const results = [];
function log(name, ok, detail) {
  results.push({ name, ok });
  console.log(`${ok ? "✓" : "✗"} ${name}${detail ? " — " + detail : ""}`);
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const target = await (
  await fetch(`${DEBUG}/json/new?${encodeURIComponent("about:blank")}`, { method: "PUT" })
).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
let seq = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
});
await new Promise((res, rej) => {
  ws.addEventListener("open", res);
  ws.addEventListener("error", rej);
});
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = ++seq;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });

await send("Page.enable");
await send("Runtime.enable");

async function evalJs(expression) {
  const r = await send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (r.result?.exceptionDetails) {
    return "__EXC__" + JSON.stringify(r.result.exceptionDetails).slice(0, 300);
  }
  return r.result?.result?.value;
}
async function goto(url) {
  await send("Page.navigate", { url });
  await sleep(900);
}
async function waitFor(expr, timeout = 15000, interval = 200) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeout) {
    if ((await evalJs(expr)) === true) return true;
    await sleep(interval);
  }
  return false;
}

await goto(`${ORIGIN}/login`);
await evalJs(`localStorage.setItem('token', ${JSON.stringify(token)})`);

const cfg = {
  requirement: {
    nodeId: process.env.CDP_R1,
    path: `/projects/${pid}/requirements`,
    expectLabel: process.env.CDP_EXPECT_LABEL || "前置需求-回填验证",
  },
  defect: {
    nodeId: process.env.CDP_D1,
    path: `/projects/${pid}/defects`,
    expectLabel: process.env.CDP_EXPECT_LABEL || "前置需求-回填验证",
  },
}[mode];

await goto(`${ORIGIN}${cfg.path}`);
// 若默认是看板视图，切回列表
await evalJs(
  `(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.trim().includes('列表')); if (b) b.click(); return true; })()`,
);

const rowFound = await waitFor(
  `document.body.innerText.includes(${JSON.stringify(cfg.nodeId)})`,
  20000,
);
log(`[${mode}] 列表找到行 ${cfg.nodeId}`, rowFound);
if (!rowFound) {
  process.exitCode = 1;
  process.exit();
}

const opened = await evalJs(
  `(() => { const row = [...document.querySelectorAll('tr')].find(r => r.innerText.includes(${JSON.stringify(cfg.nodeId)})); if (!row) return 'no-row'; const btn = [...row.querySelectorAll('button')].find(b => b.textContent.trim() === '编辑' && b.offsetParent !== null); if (!btn) return 'no-btn'; btn.click(); return 'clicked'; })()`,
);
log(`[${mode}] 点击「编辑」打开表单`, opened === "clicked", String(opened));

const prefilled = await waitFor(
  `(() => { const c = document.querySelector('.node-picker .chip-label'); return !!c && c.textContent.includes(${JSON.stringify(cfg.expectLabel)}); })()`,
  15000,
);
log(`[${mode}] 回填现有依赖（chip 显示「${cfg.expectLabel}」）`, prefilled);

if (mode === "requirement" && prefilled) {
  await evalJs(`document.querySelector('.node-picker .chip-remove').click()`);
  const removed = await waitFor(`!document.querySelector('.node-picker .chip-label')`, 5000);
  log(`[${mode}] 点击 × 移除 chip`, removed);
  const saved = await evalJs(
    `(() => { const btns = [...document.querySelectorAll('button')].filter(b => b.textContent.trim() === '保存' && b.offsetParent !== null); if (!btns.length) return 'no-save-btn'; btns[btns.length - 1].click(); return 'saved'; })()`,
  );
  log(`[${mode}] 点击「保存」`, saved === "saved", String(saved));
  await sleep(1800);
}

process.exitCode = results.every((r) => r.ok) ? 0 : 1;
process.exit();
