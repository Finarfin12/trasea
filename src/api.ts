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
	/** circles, ellipses and rectangles as pure shapes */
	detectPrimitives: boolean;
	symmetry: Symmetry;
	/** warn about suspected text areas */
	detectText: boolean;
}

export type Symmetry = "off" | "auto" | "vertical" | "horizontal";

/** A suspected text area in source pixels (preview only, never exported). */
export interface TextRegion {
	x: number;
	y: number;
	w: number;
	h: number;
	glyphCount: number;
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
	primitives: number;
	symmetryApplied: string | null;
	textRegions: TextRegion[];
}

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp"];

export const processImage = (path: string, params: Options): Promise<Processed> => invoke<Processed>("process", { path, params });
export interface ExportOptions {
	linesAs: "stroke" | "shape";
	trapPt: number;
}

export type ExportKind = "svg" | "pdf" | "ase";
export const saveFile = (kind: ExportKind, path: string, options: ExportOptions): Promise<void> => invoke<void>("save_file", { kind, path, options });
export const sidecarAvailable = (): Promise<boolean> => invoke<boolean>("sidecar_available");
export const licenses = (): Promise<string> => invoke<string>("third_party_licenses");

export async function pickImage(): Promise<string | null> {
	const r = await open({ multiple: false, directory: false, filters: [{ name: "Gambar", extensions: IMAGE_EXTENSIONS }] });
	return typeof r === "string" ? r : null;
}

export async function pickSavePath(kind: ExportKind, suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: kind === "ase" ? "Adobe Swatch Exchange" : kind.toUpperCase(), extensions: [kind] }] });
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
