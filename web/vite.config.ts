import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The development loop: Vite serves the app with HMR on 5173 and proxies
// API and socket traffic to the Rust server on 8080. Nothing is embedded
// in the binary until `npm run build`.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:8080",
      "/ws": { target: "ws://localhost:8080", ws: true },
    },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
