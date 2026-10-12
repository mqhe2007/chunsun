// Console 需求/缺陷列表+看板「被阻塞」标签随前置完成实时隐去 —— 真实 UI 验证（headless Chrome + CDP，零第三方依赖）。
//
// 背景（缺陷 -TC-qo2MSGNB / 修复需求 W5En71uyhaRT）：列表页此前把依赖边 target 一律计为
// 「被阻塞」，前置完成后标签不消失；本脚本走真实浏览器路径验证修复：
//   1. 需求列表：B（被未完成 A1 阻塞）显示「被阻塞」；E（被未完成 C 阻塞）显示；C 自身不显示
//   2. 缺陷列表：D（被未完成 A2 阻塞，跨类型）显示「被阻塞」
//   3. 看板：B/E 卡片显示、C 卡片不显示；A1 置 completed 后点「刷新」，B 卡片标签实时隐去、E 保持
//   4. 切回列表：B 行标签已隐去、E 行保持
//   5. 缺陷列表：A2 置 completed 后点「刷新」，D 行标签实时隐去
//
// 前置（本机 dev）：
//   - 后端：PORT=11112 cargo run --manifest-path packages/backend/Cargo.toml
//   - console dev：corepack pnpm --filter console dev（11114，应用在 /console/ 下）
//   - headless Chrome：
//       "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new \
//         --remote-debugging-port=9333 --user-data-dir=$TMPDIR/chrome-cdp-profile about:blank
//   - 脚本自引导：登录 → 建项目 → 生成 Secret Key → 建需求/缺陷与依赖（无需手工准备数据）
//
// 用法：
//   CHUNSUN_EMAIL=... CHUNSUN_PASSWORD=... node packages/console/scripts/verify-blocked-badge.mjs
//   （可选 CHUNSUN_BASE / CDP_DEBUG_URL / CDP_CONSOLE_ORIGIN）
//
// 退出码 0 = 全部断言通过。
const API = process.env.CHUNSUN_BASE || "http://127.0.0.1:11112/api/v1";
const DEBUG = process.env.CDP_DEBUG_URL || "http://127.0.0.1:9333";
const ORIGIN = process.env.CDP_CONSOLE_ORIGIN || "http://127.0.0.1:11114/console";
const email = process.env.CHUNSUN_EMAIL;
const password = process.env.CHUNSUN_PASSWORD;
if (!email || !password) {
  console.error("需要 CHUNSUN_EMAIL / CHUNSUN_PASSWORD（本地 dev 账号，例如 e2e-kcli@test.local）");
  process.exit(1);
}

const results = [];
function log(name, ok, detail) {
  results.push({ name, ok });
  console.log(`${ok ? "✓" : "✗"} ${name}${detail ? " — " + detail : ""}`);
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ── API 引导（登录 / 项目 / Secret Key / 测试数据）─────────
async function api(method, path, body, token) {
  const res = await fetch(`${API}${path}`, {
    method,
    headers: {
      "content-type": "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await res.json().catch(() => ({}));
  if (!res.ok || json.success === false) {
    throw new Error(`${method} ${path} → ${res.status} ${JSON.stringify(json).slice(0, 300)}`);
  }
  return json.data;
}

const login = await api("POST", "/auth/login", { email, password });
const token = login.token;
log("登录并拿到 token", typeof token === "string" && token.length > 0);

const project = await api("POST", "/projects", {
  name: `阻塞标签 e2e ${Date.now()}`,
  description: "blocked badge e2e",
}, token);
const pid = project.id;
await api("POST", `/projects/${pid}/secret-key/generate`, {}, token);
log(`项目创建 + Secret Key（${pid}）`, true);

// 描述文案避开「被阻塞」字面量，避免行内文本断言被描述误命中。
// A1 → B（需求侧前置，后置完成）；C → E（前置保持未完成）；A2 → D（缺陷侧跨类型前置，后置完成）。
const reqA1 = await api("POST", `/projects/${pid}/requirements`, { description: "阻塞源A1-稍后置完成" }, token);
const reqB = await api("POST", `/projects/${pid}/requirements`, {
  description: "目标B-依赖A1",
  blockedBy: [{ kind: "requirement", id: reqA1.id }],
}, token);
const reqC = await api("POST", `/projects/${pid}/requirements`, { description: "阻塞源C-保持未完成" }, token);
const reqE = await api("POST", `/projects/${pid}/requirements`, {
  description: "目标E-依赖C",
  blockedBy: [{ kind: "requirement", id: reqC.id }],
}, token);
const reqA2 = await api("POST", `/projects/${pid}/requirements`, { description: "阻塞源A2-稍后置完成" }, token);
const defD = await api("POST", `/projects/${pid}/defects`, {
  description: "目标缺陷D-依赖A2",
  blockedBy: [{ kind: "requirement", id: reqA2.id }],
}, token);
log("测试数据：A1→B、C→E、A2→D（跨类型）", [reqB.id, reqE.id, defD.id].every(Boolean));

// ── CDP ─────────────────────────────────────────────────
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

/** 列表行内是否出现「被阻塞」徽标（行内其余文案已避开该字面量）。 */
const rowCheck = (id) =>
  `(() => { const row = [...document.querySelectorAll('tr')].find(r => r.innerText.includes(${JSON.stringify(id)})); if (!row) return 'no-row'; return row.innerText.includes('被阻塞'); })()`;
/** 看板卡片内是否出现「被阻塞」徽标。 */
const cardCheck = (id) =>
  `(() => { const card = [...document.querySelectorAll('.req-card')].find(c => c.innerText.includes(${JSON.stringify(id)})); if (!card) return 'no-card'; return card.innerText.includes('被阻塞'); })()`;
const clickRefresh = `(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.trim() === '刷新' && x.offsetParent !== null); if (!b) return false; b.click(); return true; })()`;
const clickListView = `(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.trim().includes('列表')); if (b) b.click(); return true; })()`;
const clickBoardView = `(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.trim().includes('看板')); if (b) b.click(); return true; })()`;

await goto(`${ORIGIN}/login`);
await evalJs(`localStorage.setItem('token', ${JSON.stringify(token)})`);

// 1) 需求列表：B / E 显示，C 不显示（前置均未完成）
await goto(`${ORIGIN}/projects/${pid}/requirements`);
await evalJs(clickListView);
log("需求列表加载（找到目标B行）", await waitFor(`document.body.innerText.includes(${JSON.stringify(reqB.id)})`, 20000));
log("B（前置A1未完成）显示「被阻塞」", await waitFor(`(${rowCheck(reqB.id)}) === true`));
log("E（前置C未完成）显示「被阻塞」", await waitFor(`(${rowCheck(reqE.id)}) === true`));
log("C（无前置）不显示「被阻塞」", await waitFor(`(${rowCheck(reqC.id)}) === false`));

// 2) 缺陷列表：D 显示（前置 A2 未完成，跨类型）
await goto(`${ORIGIN}/projects/${pid}/defects`);
log("缺陷列表加载（找到目标缺陷D行）", await waitFor(`document.body.innerText.includes(${JSON.stringify(defD.id)})`, 20000));
log("D（前置A2未完成，跨类型）显示「被阻塞」", await waitFor(`(${rowCheck(defD.id)}) === true`));

// 3) 需求看板：B / E 卡片显示，C 卡片不显示
await goto(`${ORIGIN}/projects/${pid}/requirements`);
await evalJs(clickBoardView);
log("需求看板加载（找到B卡片）", await waitFor(`(${cardCheck(reqB.id)}) !== 'no-card'`, 20000));
log("看板 B 卡片显示「被阻塞」", await waitFor(`(${cardCheck(reqB.id)}) === true`));
log("看板 E 卡片显示「被阻塞」", await waitFor(`(${cardCheck(reqE.id)}) === true`));
log("看板 C 卡片不显示「被阻塞」", await waitFor(`(${cardCheck(reqC.id)}) === false`));

// 4) 前置 A1 置 completed → 看板点「刷新」，B 卡片标签实时隐去、E 保持
await api("PATCH", `/projects/${pid}/requirements/${reqA1.id}`, { status: "completed" }, token);
await evalJs(clickRefresh);
log("看板刷新：B 卡片标签隐去", await waitFor(`(${cardCheck(reqB.id)}) === false`));
log("看板刷新：E 卡片（前置C仍未完成）标签保持", await waitFor(`(${cardCheck(reqE.id)}) === true`));

// 5) 切回列表：B 行标签已隐去、E 行保持
await evalJs(clickListView);
log("列表 B 行标签隐去", await waitFor(`(${rowCheck(reqB.id)}) === false`));
log("列表 E 行标签保持", await waitFor(`(${rowCheck(reqE.id)}) === true`));

// 6) 缺陷列表：A2 置 completed 后点「刷新」，D 行标签实时隐去
await goto(`${ORIGIN}/projects/${pid}/defects`);
await waitFor(`document.body.innerText.includes(${JSON.stringify(defD.id)})`, 20000);
await evalJs(clickRefresh);
log("缺陷列表刷新（A2 未完成）：D 标签保持", await waitFor(`(${rowCheck(defD.id)}) === true`));
await api("PATCH", `/projects/${pid}/requirements/${reqA2.id}`, { status: "completed" }, token);
await evalJs(clickRefresh);
log("A2 completed 后刷新：缺陷D标签隐去", await waitFor(`(${rowCheck(defD.id)}) === false`));

process.exitCode = results.every((r) => r.ok) ? 0 : 1;
process.exit();
