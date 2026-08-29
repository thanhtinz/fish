import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built files from disk, so assets must be referenced relatively.
export default defineConfig({
  plugins: [react()],
  base: "./",
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
