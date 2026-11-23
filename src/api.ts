import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

export type ProductionMode = "standard" | "screenPrint" | "cutEmbroidery";

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
	/** the designer's own palette (#RRGGBB), null = automatic */
	customPalette: string[] | null;
	/** explicit colour merges [[fromHex, toHex], ...] */
	paletteMerges: Array<[string, string]>;
	/** node budget, null = automatic */
	targetNodes: number | null;
	/** correction brush strokes in source pixels */
	brushStrokes: BrushStroke[];
	/** production profile */
	productionMode: ProductionMode;
	/** minimum stroke width in points (0 = disabled) */
	minStrokePt: number;
	/** strip background layer (CutEmbroidery) */
	stripBackground: boolean;
	/** add 4 corner registration marks (ScreenPrint) */
	registrationMarks: boolean;
}

export interface BrushStroke {
	kind: "detail" | "smooth";
	/** radius in source pixels */
	radius: number;
	points: Array<[number, number]>;
}

/** Node density per grid cell (row-major). Preview overlay only; never exported. */
export interface NodeHeatmap {
	cols: number;
	rows: number;
	cells: number[];
	maxCell: number;
	hotCells: number;
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
	heatmap: NodeHeatmap;
	/** smoothness 0..100 that was really used */
	effectiveSmooth: number;
	budgetMet: boolean | null;
	/** loading and cleaning were not run again */
	reused: boolean;
}

export interface ExportOptions {
	linesAs: "stroke" | "shape";
	trapPt: number;
	registrationMarks: boolean;
	stripBackground: boolean;
}

export type BatchStatus = "ok" | "failed" | "cancelled";

export interface BatchItemReport {
	inputPath: string;
	fileName: string;
	status: BatchStatus;
	error: string | null;
	svgPath: string | null;
	colors: number;
	paths: number;
	nodes: number;
	mae: number | null;
	warnings: string[];
	secs: number;
}

export interface BatchProgress {
	done: number;
	total: number;
	currentFile: string;
	stage: string;
	itemReport: BatchItemReport | null;
}

export interface BatchSummary {
	total: number;
	succeeded: number;
	failed: number;
	cancelled: number;
	totalSecs: number;
	items: BatchItemReport[];
	reportCsvPath: string | null;
	reportMdPath: string | null;
}

export interface UiBatchConfig {
	inputs: string[];
	outputDir: string;
	params: Options;
	export: ExportOptions;
	writePdf: boolean;
	writeAse: boolean;
	writeSeparations: boolean;
}

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp"];

export const processImage = (path: string, params: Options): Promise<Processed> => invoke<Processed>("process", { path, params });
/** Eyedropper: median colour around one pixel of the open picture (source pixels). */
export const pickColor = (x: number, y: number): Promise<string> => invoke<string>("pick_color", { x, y });

export type ExportKind = "svg" | "pdf" | "ase";
export const saveFile = (kind: ExportKind, path: string, options: ExportOptions): Promise<void> => invoke<void>("save_file", { kind, path, options });
export const saveSeparations = (dir: string, baseName: string, kind: "svg" | "pdf" | "both", options: ExportOptions): Promise<string[]> =>
	invoke<string[]>("save_separations", { dir, baseName, kind, options });

export const scanFolder = (dir: string, includeSubfolders: boolean): Promise<string[]> => invoke<string[]>("scan_folder", { dir, includeSubfolders });
export const collectDroppedImages = (paths: string[], includeSubfolders = false): Promise<string[]> => invoke<string[]>("collect_dropped_images", { paths, includeSubfolders });
export const runBatch = (config: UiBatchConfig): Promise<BatchSummary> => invoke<BatchSummary>("run_batch", { config });
export const cancelBatch = (): Promise<void> => invoke<void>("cancel_batch");
export const openFolder = (dir: string): Promise<void> => invoke<void>("open_folder", { dir });

export const sidecarAvailable = (): Promise<boolean> => invoke<boolean>("sidecar_available");
export const licenses = (): Promise<string> => invoke<string>("third_party_licenses");

export async function pickImage(): Promise<string | null> {
	const r = await open({ multiple: false, directory: false, filters: [{ name: "Gambar", extensions: IMAGE_EXTENSIONS }] });
	return typeof r === "string" ? r : null;
}

export async function pickImages(): Promise<string[]> {
	const r = await open({ multiple: true, directory: false, filters: [{ name: "Gambar", extensions: IMAGE_EXTENSIONS }] });
	if (Array.isArray(r)) return r.filter((x): x is string => typeof x === "string");
	return typeof r === "string" ? [r] : [];
}

export async function pickFolder(title?: string): Promise<string | null> {
	const r = await open({ multiple: false, directory: true, title });
	return typeof r === "string" ? r : null;
}

export async function pickSavePath(kind: ExportKind, suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: kind === "ase" ? "Adobe Swatch Exchange" : kind.toUpperCase(), extensions: [kind] }] });
}

export const onProgress = (fn: (stage: string, fraction: number) => void) => listen<{ stage: string; fraction: number }>("progress", e => fn(e.payload.stage, e.payload.fraction));
export const onBatchProgress = (fn: (prog: BatchProgress) => void) => listen<BatchProgress>("batch-progress", e => fn(e.payload));

/** Files or folders dropped on the window (Tauri reports real paths). */
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