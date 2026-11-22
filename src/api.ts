import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

export interface Options {
	colors: number;
	/** 0..100 */
	smooth: number;
	keepLines: boolean;
	aiClean: boolean;
}

export interface Processed {
	svg: string;
	original: string;
	width: number;
	height: number;
	paths: number;
	nodes: number;
	bytes: number;
	mae: number | null;
	notes: string[];
	palette: string[];
	usedAi: boolean;
	secs: number;
	thinPixels: number;
}

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp"];

export const processImage = (path: string, params: Options): Promise<Processed> => invoke<Processed>("process", { path, params });
export const saveFile = (kind: "svg" | "pdf", path: string): Promise<void> => invoke<void>("save_file", { kind, path });
export const sidecarAvailable = (): Promise<boolean> => invoke<boolean>("sidecar_available");
export const licenses = (): Promise<string> => invoke<string>("third_party_licenses");

export async function pickImage(): Promise<string | null> {
	const r = await open({ multiple: false, directory: false, filters: [{ name: "Gambar", extensions: IMAGE_EXTENSIONS }] });
	return typeof r === "string" ? r : null;
}

export async function pickSavePath(kind: "svg" | "pdf", suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: kind.toUpperCase(), extensions: [kind] }] });
}

export const onProgress = (fn: (stage: string, fraction: number) => void) => listen<{ stage: string; fraction: number }>("progress", e => fn(e.payload.stage, e.payload.fraction));

/** Files dropped on the window (Tauri reports real paths). */
export const onDrop = async (fn: (paths: string[]) => void, setOver: (over: boolean) => void) =>
	getCurrentWebview().onDragDropEvent(e => {
		const t = e.payload.type;
		if (t === "over" || t === "enter") setOver(true);
		else if (t === "leave") setOver(false);
		else if (t === "drop") {
			setOver(false);
			fn(e.payload.paths);
		}
	});
