import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import pkg from "./package.json" with { type: "json" };

// Tauri expects a fixed dev port and the build in ./dist
export default defineConfig({
	plugins: [react()],
	define: {
		__APP_VERSION__: JSON.stringify(pkg.version),
	},
	clearScreen: false,
	server: { port: 1420, strictPort: true, host: "localhost" },
	build: { target: "es2021", outDir: "dist", emptyOutDir: true },
});
