#!/usr/bin/env node
/**
 * 本地同源入口：浏览器只记 :11111。
 *  /api、/cli → 后端 / 本地 CLI 产物
 *  /console/* → console Vite（包名即路径）
 *  其余 → website Vite
 */
import { spawn } from "node:child_process";
import {
  createReadStream,
  existsSync,
  readFileSync,
  statSync,
} from "node:fs";
import http from "node:http";
import net from "node:net";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const CLI_DOWNLOAD_URL_PLACEHOLDER = "__CHUNSUN_CLI_DOWNLOAD_URL__";

function envNum(name, fallback) {
  const raw = process.env[name];
  const n = raw ? Number(raw) : fallback;
  return Number.isFinite(n) ? n : fallback;
}

const gatewayPort = envNum("PORT", 11111);
const apiPort = envNum("API_PORT", 11112);
const websitePort = envNum("WEBSITE_PORT", 11113);
const consolePort = envNum("CONSOLE_PORT", 11114);

const websiteOrigin = `http://127.0.0.1:${websitePort}`;
const consoleOrigin = `http://127.0.0.1:${consolePort}`;
const apiOrigin = `http://127.0.0.1:${apiPort}`;

function isConsolePath(pathname) {
  return pathname === "/console" || pathname.startsWith("/console/");
}

function waitForPort(port, label) {
  const deadline = Date.now() + 30_000;
  return new Promise((resolveWait, reject) => {
    const tryOnce = () => {
      const socket = net.connect({ host: "127.0.0.1", port }, () => {
        socket.end();
        resolveWait();
      });
      socket.on("error", () => {
        socket.destroy();
        if (Date.now() > deadline) {
          reject(new Error(`[dev-web] 等待 ${label} :${port} 超时`));
          return;
        }
        setTimeout(tryOnce, 150);
      });
    };
    tryOnce();
  });
}

function proxy(req, res, targetOrigin, overridePath) {
  const url = new URL(overridePath ?? req.url ?? "/", targetOrigin);
  const headers = { ...req.headers, host: new URL(targetOrigin).host };
  const method = overridePath ? "GET" : (req.method ?? "GET");
  const idempotent = method === "GET" || method === "HEAD";

  const send = (attempt = 0) => {
    const proxyReq = http.request(url, { method, headers }, proxyRes => {
      res.writeHead(proxyRes.statusCode ?? 502, proxyRes.headers);
      proxyRes.pipe(res);
    });
    proxyReq.on("error", err => {
      const retryable =
        err.code === "ECONNREFUSED" || err.code === "ECONNRESET" || err.code === "ETIMEDOUT";
      if (retryable && idempotent && attempt < 20 && !res.headersSent) {
        setTimeout(() => send(attempt + 1), 150);
        return;
      }
      if (!res.headersSent) {
        res.writeHead(502, { "content-type": "text/plain; charset=utf-8" });
      }
      res.end(`[chunsun] 网关无法连接 ${targetOrigin}: ${err.message}\n`);
    });
    if (overridePath || idempotent) {
      proxyReq.end();
    } else {
      req.pipe(proxyReq);
    }
  };
  send();
}

function serveCli(req, res, pathname) {
  const cliMount = "/cli";
  let relativePath = decodeURIComponent(pathname.slice(cliMount.length).replace(/^\//, ""));
  if (!relativePath) {
    res.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
    res.end("Not Found\n");
    return;
  }

  const host = req.headers.host ?? `localhost:${gatewayPort}`;
  const protoHeader = req.headers["x-forwarded-proto"];
  const proto = typeof protoHeader === "string" ? protoHeader.split(",")[0].trim() : "http";
  const cliBaseUrl = `${proto}://${host}${cliMount}`;
  const cliScriptsDir = resolve(ROOT, "packages/cli/scripts");
  const cliDistDir = resolve(ROOT, "packages/cli/dist");

  if (relativePath === "install.sh" || relativePath === "install.ps1") {
    const scriptPath = resolve(cliScriptsDir, relativePath);
    if (!existsSync(scriptPath)) {
      res.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
      res.end(`Not Found: ${relativePath}\n`);
      return;
    }
    const body = readFileSync(scriptPath, "utf-8").replaceAll(
      CLI_DOWNLOAD_URL_PLACEHOLDER,
      cliBaseUrl,
    );
    res.writeHead(200, {
      "content-type": "text/plain; charset=utf-8",
      "cache-control": "no-store",
    });
    if (req.method === "HEAD") {
      res.end();
      return;
    }
    res.end(body);
    return;
  }

  const filePath = resolve(cliDistDir, relativePath);
  const relToDist = relative(cliDistDir, filePath);
  if (
    !relToDist ||
    relToDist.startsWith(`..${sep}`) ||
    relToDist === ".." ||
    relToDist.includes(`..${sep}`)
  ) {
    res.writeHead(400, { "content-type": "text/plain; charset=utf-8" });
    res.end("Bad Request\n");
    return;
  }

  if (!existsSync(filePath) || !statSync(filePath).isFile()) {
    res.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
    res.end(`[chunsun] CLI 产物不存在: ${relativePath}\n请先执行: pnpm run cli:dist\n`);
    return;
  }

  res.writeHead(200, {
    "content-type": "application/octet-stream",
    "cache-control": "no-store",
  });
  if (req.method === "HEAD") {
    res.end();
    return;
  }
  createReadStream(filePath).pipe(res);
}

const server = http.createServer((req, res) => {
  const pathname = (req.url ?? "/").split("?")[0] || "/";
  if (pathname === "/api" || pathname.startsWith("/api/")) {
    proxy(req, res, apiOrigin);
    return;
  }
  if (pathname === "/cli" || pathname.startsWith("/cli/")) {
    serveCli(req, res, pathname);
    return;
  }
  if (isConsolePath(pathname)) {
    proxy(req, res, consoleOrigin);
    return;
  }
  proxy(req, res, websiteOrigin);
});

function spawnInherit(command, args, extraEnv = {}) {
  const child = spawn(command, args, {
    cwd: ROOT,
    stdio: "inherit",
    env: { ...process.env, ...extraEnv },
    shell: false,
  });
  child.on("exit", code => {
    if (shuttingDown) return;
    console.error(`[dev-web] ${command} ${args.join(" ")} 退出 (${code ?? "?"})`);
    shutdown(code ?? 1);
  });
  return child;
}

let shuttingDown = false;
const children = [];

function shutdown(code) {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const child of children) {
    if (!child.killed) child.kill("SIGTERM");
  }
  server.close(() => process.exit(code));
  setTimeout(() => process.exit(code), 2000).unref();
}

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

children.push(
  spawnInherit("pnpm", ["run", "be:dev"], { PORT: String(apiPort), API_PORT: String(apiPort) }),
);
children.push(spawnInherit("pnpm", ["--filter", "website", "dev"]));
children.push(spawnInherit("pnpm", ["--filter", "console", "dev"]));

Promise.all([
  waitForPort(websitePort, "website"),
  waitForPort(consolePort, "console"),
])
  .then(() => {
    server.listen(gatewayPort, "127.0.0.1", () => {
      console.log(
        `[dev-web] 浏览器入口 http://127.0.0.1:${gatewayPort}  (website :${websitePort} · console :${consolePort} · api :${apiPort})`,
      );
    });
  })
  .catch(err => {
    console.error(err.message ?? err);
    shutdown(1);
  });
