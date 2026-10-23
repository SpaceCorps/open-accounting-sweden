import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    // `vp dev` talks to a server started with: cargo run -p oas-server
    proxy: { "/api": "http://127.0.0.1:8080", "/health": "http://127.0.0.1:8080" },
  },
  build: { outDir: "dist", sourcemap: false },
});
