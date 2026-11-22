// Downloads the Real-ESRGAN ncnn Vulkan release (v0.2.5.0, Windows) and copies the files Vektora needs into
// src-tauri/resources/realesrgan/ (executable, OpenMP runtime, and the anime x4 model only).
// Needs Node 22+ and `tar` (bundled with Windows 10+). Run once before `tauri build`: node scripts/fetch-sidecar.mjs
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const URL = "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesrgan-ncnn-vulkan-20220424-windows.zip";
const dest = join(import.meta.dirname, "..", "src-tauri", "resources", "realesrgan");
if (existsSync(join(dest, "realesrgan-ncnn-vulkan.exe"))) {
	console.log("sidecar already present:", dest);
	process.exit(0);
}
const tmp = mkdtempSync(join(tmpdir(), "vektora-esrgan-"));
try {
	const res = await fetch(URL);
	if (!res.ok) throw new Error(`download failed: HTTP ${res.status}`);
	const zip = join(tmp, "esrgan.zip");
	writeFileSync(zip, Buffer.from(await res.arrayBuffer()));
	execFileSync("tar", ["-xf", zip, "-C", tmp]);
	mkdirSync(join(dest, "models"), { recursive: true });
	for (const f of ["realesrgan-ncnn-vulkan.exe", "vcomp140.dll"]) cpSync(join(tmp, f), join(dest, f));
	for (const f of ["realesrgan-x4plus-anime.bin", "realesrgan-x4plus-anime.param"]) cpSync(join(tmp, "models", f), join(dest, "models", f));
	console.log("sidecar installed in", dest);
} finally {
	rmSync(tmp, { recursive: true, force: true });
}
