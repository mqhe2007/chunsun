import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";
import { loadEnv } from "vite";
import tailwindcss from "@tailwindcss/vite";
import VueRouter from "vue-router/vite";
import vue from "@vitejs/plugin-vue";

const pkgDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(pkgDir, "../..");
const appVersion = JSON.parse(readFileSync(resolve(rootDir, "package.json"), "utf-8")).version as string;

export default defineConfig(({ mode, command }) => {
  const env = loadEnv(mode, rootDir, "");
  const consolePort = Number(env.CONSOLE_PORT || "11114");
  const backendPort = env.API_PORT || "11112";

  return {
    base: "/console/",
    envDir: rootDir,
    define: {
      __APP_VERSION__: JSON.stringify(appVersion),
    },
    plugins: [
      tailwindcss(),
      VueRouter({
        routesFolder: "src/pages",
        dts: "./typed-router.d.ts",
        extensions: [".vue"],
      }),
      vue(),
    ],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
      },
    },
    optimizeDeps: {
      exclude: ["@chunsun/web-shared"],
    },
    server: {
      host: "127.0.0.1",
      port: consolePort,
      strictPort: true,
      hmr: command === "serve" ? { host: "127.0.0.1", port: consolePort, clientPort: consolePort } : undefined,
      fs: { allow: [rootDir] },
      proxy: {
        "/api": {
          target: `http://127.0.0.1:${backendPort}`,
          changeOrigin: true,
        },
      },
    },
    build: {
      assetsDir: "assets",
    },
    test: {
      environment: "node",
    },
  };
});
