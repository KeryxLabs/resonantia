import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

const host = process.env.TAURI_DEV_HOST;

const defaultAllowedHosts = ["localhost", "127.0.0.1", "resonantia.levare.cloud"];
/** @type {string[]} */
const envAllowedHosts = (process.env.VITE_ALLOWED_HOSTS ?? "")
  .split(",")
  .map((/** @type {string} */ value) => value.trim())
  .filter((/** @type {string} */ value) => value.length > 0);
const allowedHosts = Array.from(new Set([...defaultAllowedHosts, ...envAllowedHosts]));

/**
 * @param {unknown} chunk
 * @returns {Uint8Array}
 */
function toUint8Array(chunk) {
  if (chunk instanceof Uint8Array) {
    return chunk;
  }

  if (typeof chunk === "string") {
    return new TextEncoder().encode(chunk);
  }

  return new TextEncoder().encode(String(chunk));
}

/**
 * @param {any} req
 * @returns {Promise<Uint8Array | undefined>}
 */
function readRequestBody(req) {
  return new Promise((resolve, reject) => {
    /** @type {Uint8Array[]} */
    const chunks = [];

    req.on(
      "data",
      /** @param {unknown} chunk */ (chunk) => {
      chunks.push(toUint8Array(chunk));
      },
    );

    req.on("end", () => {
      if (chunks.length === 0) {
        resolve(undefined);
        return;
      }

      let totalLength = 0;
      for (const chunk of chunks) {
        totalLength += chunk.byteLength;
      }

      const payload = new Uint8Array(totalLength);
      let offset = 0;
      for (const chunk of chunks) {
        payload.set(chunk, offset);
        offset += chunk.byteLength;
      }

      resolve(payload);
    });

    req.on("error", reject);
  });
}

/** @returns {import("vite").Plugin} */
function devGatewayProxyPlugin() {
  const proxyPrefixes = ["/__gateway_proxy__", "/gateway_proxy", "/gateway"];
  const defaultGatewayUpstream = "http://resonantia-gateway:8060";
  const gatewayUpstreamRaw = (defaultGatewayUpstream).trim();
  const gatewayUpstream = gatewayUpstreamRaw.endsWith("/") ? gatewayUpstreamRaw.slice(0, -1) : gatewayUpstreamRaw;

  /**
   * @param {string} path
   * @returns {string | null}
   */
  const matchProxyPrefix = (path) => {
    for (const prefix of proxyPrefixes) {
      if (path === prefix || path.startsWith(`${prefix}/`)) {
        return prefix;
      }
    }

    return null;
  };

  /** @param {import("vite").ViteDevServer | any} server */
  const attachProxyMiddleware = (server) => {
    /** @type {(req: any, res: any) => Promise<void>} */
    const handler = async (req, res) => {
      const inbound = /** @type {any} */ (req);
      const outbound = /** @type {any} */ (res);
      const requestUrl = new URL(inbound.url ?? "/", "http://localhost");
      const path = requestUrl.pathname;
      const matchedPrefix = matchProxyPrefix(path);

      if (!matchedPrefix) {
        outbound.statusCode = 404;
        outbound.setHeader("Content-Type", "text/plain; charset=utf-8");
        outbound.end("gateway proxy route not found");
        return;
      }

      let upstreamBase;
      try {
        upstreamBase = new URL(gatewayUpstream);
      } catch {
        outbound.statusCode = 400;
        outbound.setHeader("Content-Type", "text/plain; charset=utf-8");
        outbound.end("gateway proxy upstream is invalid");
        return;
      }

      if (upstreamBase.protocol !== "http:" && upstreamBase.protocol !== "https:") {
        outbound.statusCode = 400;
        outbound.setHeader("Content-Type", "text/plain; charset=utf-8");
        outbound.end("gateway proxy upstream must use http or https");
        return;
      }

      const proxiedPath = path.slice(matchedPrefix.length) || "/";
      const destination = new URL(`${proxiedPath}${requestUrl.search}`, upstreamBase);

      const method = (inbound.method ?? "GET").toUpperCase();
      const hasBody = method !== "GET" && method !== "HEAD";

      try {
        const body = hasBody ? await readRequestBody(inbound) : undefined;
        const headers = new Headers();

        for (const [name, value] of Object.entries(inbound.headers ?? {})) {
          if (!value) {
            continue;
          }

          const key = name.toLowerCase();
          if (key === "host" || key === "origin" || key === "referer" || key === "connection" || key === "content-length") {
            continue;
          }

          headers.set(name, Array.isArray(value) ? value.join(", ") : value);
        }

        headers.set("x-forwarded-host", inbound.headers?.host ?? "");
        headers.set("x-forwarded-proto", "http");

        const upstream = await fetch(destination.toString(), {
          method,
          headers,
          body,
          redirect: "manual",
        });

        outbound.statusCode = upstream.status;
        upstream.headers.forEach((value, name) => {
          const key = name.toLowerCase();
          if (key === "transfer-encoding" || key === "connection" || key === "content-encoding") {
            return;
          }

          outbound.setHeader(name, value);
        });

        const payload = new Uint8Array(await upstream.arrayBuffer());
        outbound.end(payload);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        outbound.statusCode = 502;
        outbound.setHeader("Content-Type", "text/plain; charset=utf-8");
        outbound.end(`gateway proxy failed: ${message}`);
      }
    };

    server.middlewares.use((req, res, next) => {
      const requestUrl = new URL(req.url ?? "/", "http://localhost");
      if (!matchProxyPrefix(requestUrl.pathname)) {
        next();
        return;
      }

      void handler(req, res);
    });
  };

  return {
    name: "resonantia-dev-gateway-proxy",
    /** @param {import("vite").ViteDevServer} server */
    configureServer(server) {
      attachProxyMiddleware(server);
    },
    /** @param {any} server */
    configurePreviewServer(server) {
      attachProxyMiddleware(server);
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => {
  const sveltePlugins = await sveltekit();
  const normalizedSveltePlugins = Array.isArray(sveltePlugins) ? sveltePlugins : [sveltePlugins];

  return {
    plugins: [devGatewayProxyPlugin(), ...normalizedSveltePlugins],
    optimizeDeps: {
      exclude: ["@surrealdb/wasm"],
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || "127.0.0.1",
      allowedHosts,
      fs: {
        allow: [".", "./packages", ".."],
      },
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
    preview: {
      allowedHosts,
    },
  };
});
