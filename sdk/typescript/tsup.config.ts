import { defineConfig } from "tsup";

/**
 * Build configuration for the @diaspor/sdk package.
 *
 * Produces dual ESM + CJS bundles and a single `.d.ts` definition file so the
 * SDK works equally well from a modern bundler, a Node 20+ runtime, and the
 * older `require()` paths still common in Electron / serverless environments.
 *
 * No runtime dependencies are bundled — the SDK relies on the host's native
 * `fetch` and `WebSocket` implementations (available on Node 20+ and in every
 * evergreen browser).
 */
export default defineConfig({
  entry: ["src/index.ts"],
  format: ["esm", "cjs"],
  dts: true,
  sourcemap: true,
  clean: true,
  splitting: false,
  treeshake: true,
  minify: false,
  target: "es2022",
  platform: "neutral",
  outDir: "dist",
});
