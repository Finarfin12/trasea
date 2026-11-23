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
	detectPrimitives: boolean;
	symmetry: Symmetry;
	detectText: boolean;
	customPalette: string[] | null;
	paletteMerges: Array<[string, string]>;
	targetNodes: number | null;
	brushStrokes: BrushStroke[];
	productionMode: ProductionMode;
	minStrokePt: number;
	stripBackground: boolean;
	registrationMarks: boolean;
}

export interface BrushStroke {
	kind: "detail" | "smooth";
	radius: number;
	points: Array<[number, number]>;
}

export interface NodeHeatmap {
	cols: number;
	rows: number;
	cells: number[];
	maxCell: number;
	hotCells: number;
}

export type Symmetry = "off" | "auto" | "vertical" | "horizontal";

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
	effectiveSmooth: number;
	budgetMet: boolean | null;
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
	writeEps: boolean;
	writeAse: boolean;
	writeSeparations: boolean;
}

export interface LicenseInfo {
	v: number;
	id: string;
	name: string;
	tier: string;
	issued: string;
}

export interface LicenseStatus {
	active: boolean;
	info: LicenseInfo | null;
}

export interface ProjectFile {
	schemaVersion: number;
	appVersion: string;
	sourcePath: string;
	sourceSha256: string;
	options: Options;
	exportOptions: ExportOptions;
}

export interface LoadedProject {
	project: ProjectFile;
	sourceExists: boolean;
	hashMatches: boolean;
}

export interface UserPreset {
	id: string;
	name: string;
	options: Options;
	exportOptions: ExportOptions;
}

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "bmp"];

export const processImage = (path: string, params: Options): Promise<Processed> => invoke<Processed>("process", { path, params });
export const pickColor = (x: number, y: number): Promise<string> => invoke<string>("pick_color", { x, y });

export type ExportKind = "svg" | "pdf" | "eps" | "ase";
export const saveFile = (kind: ExportKind, path: string, options: ExportOptions): Promise<void> => invoke<void>("save_file", { kind, path, options });
export const saveSeparations = (dir: string, baseName: string, kind: "svg" | "pdf" | "both", options: ExportOptions): Promise<string[]> =>
	invoke<string[]>("save_separations", { dir, baseName, kind, options });

export const getLicenseStatus = (): Promise<LicenseStatus> => invoke<LicenseStatus>("license_status");
export const activateLicense = (key: string): Promise<LicenseStatus> => invoke<LicenseStatus>("activate_license", { key });
export const deactivateLicense = (): Promise<LicenseStatus> => invoke<LicenseStatus>("deactivate_license");

export const saveProjectFile = (path: string, sourcePath: string, options: Options, exportOptions: ExportOptions): Promise<void> =>
	invoke<void>("save_project", { path, sourcePath, options, exportOptions });
export const loadProjectFile = (path: string): Promise<LoadedProject> => invoke<LoadedProject>("load_project", { path });

export const listPresets = (): Promise<UserPreset[]> => invoke<UserPreset[]>("list_presets");
export const savePreset = (preset: UserPreset): Promise<UserPreset[]> => invoke<UserPreset[]>("save_preset", { preset });
export const deletePreset = (id: string): Promise<UserPreset[]> => invoke<UserPreset[]>("delete_preset", { id });
export const getSampleImagePath = (): Promise<string> => invoke<string>("sample_image_path");

export const scanFolder = (dir: string, includeSubfolders: boolean): Promise<string[]> => invoke<string[]>("scan_folder", { dir, includeSubfolders });
export const collectDroppedImages = (paths: string[], includeSubfolders = false): Promise<string[]> => invoke<string[]>("collect_dropped_images", { paths, includeSubfolders });
export const runBatch = (config: UiBatchConfig): Promise<BatchSummary> => invoke<BatchSummary>("run_batch", { config });
export const cancelBatch = (): Promise<void> => invoke<void>("cancel_batch");
export const openFolder = (dir: string): Promise<void> => invoke<void>("open_folder", { dir });

export const sidecarAvailable = (): Promise<boolean> => invoke<boolean>("sidecar_available");
export const licenses = (): Promise<string> => invoke<string>("third_party_licenses");

export async function pickImageOrProject(): Promise<string | null> {
	const r = await open({
		multiple: false,
		directory: false,
		filters: [
			{ name: "Gambar & Proyek Trasea", extensions: [...IMAGE_EXTENSIONS, "trasea"] },
			{ name: "Gambar", extensions: IMAGE_EXTENSIONS },
			{ name: "Proyek Trasea", extensions: ["trasea"] },
		],
	});
	return typeof r === "string" ? r : null;
}

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
	const label = kind === "ase" ? "Adobe Swatch Exchange" : kind === "eps" ? "Encapsulated PostScript" : kind.toUpperCase();
	return save({ defaultPath: suggested, filters: [{ name: label, extensions: [kind] }] });
}

export async function pickProjectSavePath(suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: "Proyek Trasea", extensions: ["trasea"] }] });
}

export const onProgress = (fn: (stage: string, fraction: number) => void) => listen<{ stage: string; fraction: number }>("progress", e => fn(e.payload.stage, e.payload.fraction));
export const onBatchProgress = (fn: (prog: BatchProgress) => void) => listen<BatchProgress>("batch-progress", e => fn(e.payload));

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
