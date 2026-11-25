import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

export type ProductionMode = "standard" | "screenPrint" | "cutEmbroidery" | "shading" | "centerline";
export type ColorSpace = "rgb" | "cmyk";
export type DxfUnit = "mm" | "px";
export type ThemeChoice = "dark" | "light" | "system";
export type CanvasBgChoice = "checkerLight" | "checkerDark" | "solid";

export interface PreprocessOptions {
	crop: [number, number, number, number] | null;
	rotateDeg: number;
	removeBackground: boolean;
	bgTolerance: number;
	defringePx: number;
}

export const DEFAULT_PREPROCESS: PreprocessOptions = {
	crop: null,
	rotateDeg: 0,
	removeBackground: false,
	bgTolerance: 24,
	defringePx: 1,
};

export type PathEdit =
	| { kind: "delete"; targetId: string }
	| { kind: "reassignColor"; targetId: string; toHex: string };

export type NodeEdit =
	| { kind: "moveNode"; targetId: string; subpathIdx: number; nodeIdx: number; x: number; y: number }
	| { kind: "moveHandle"; targetId: string; subpathIdx: number; nodeIdx: number; which: "in" | "out"; x: number; y: number; mirrorSmooth: boolean }
	| { kind: "deleteNode"; targetId: string; subpathIdx: number; nodeIdx: number }
	| { kind: "toggleSmooth"; targetId: string; subpathIdx: number; nodeIdx: number }
	| { kind: "simplifyPath"; targetId: string; tolerancePx: number };

export interface NodePoint {
	index: number;
	x: number;
	y: number;
	handleIn: [number, number] | null;
	handleOut: [number, number] | null;
	isSmooth: boolean;
}

export interface SubpathNodes {
	subpathIndex: number;
	closed: boolean;
	nodes: NodePoint[];
}

export interface PathNodeGeometry {
	targetId: string;
	colorHex: string;
	totalNodes: number;
	subpaths: SubpathNodes[];
}

export interface SpotColorMapping {
	hex: string;
	spotName: string;
}

export interface PaletteProofItem {
	hex: string;
	proofHex: string;
	outOfGamut: boolean;
}

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
	sablonKnockout: boolean;
	preprocess: PreprocessOptions;
	centerlineInvert: boolean;
	centerlineStrokePt: number;
	pathEdits: PathEdit[];
	nodeEdits: NodeEdit[];
	/** Fingerprint of the unedited trace the path/node edits were made on (edits for another trace are skipped). */
	editFingerprint: string | null;
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
	paletteProof: PaletteProofItem[];
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
	/** Fingerprint of the unedited trace; path/node edits belong to exactly this trace. */
	traceFingerprint: string;
	/** True when the edits sent with this run were skipped because they were made on a different trace. */
	editsStale: boolean;
}

export interface RawImagePreview {
	dataUrl: string;
	width: number;
	height: number;
}

export interface ExportOptions {
	linesAs: "stroke" | "shape";
	trapPt: number;
	registrationMarks: boolean;
	stripBackground: boolean;
	colorSpace: ColorSpace;
	dxfUnit: DxfUnit;
	dxfTargetWidthMm: number | null;
	dieCutContour: boolean;
	dieCutOffsetMm: number;
	dieCutSpotName: string;
	dieCutBridgeIslands: boolean;
	spotColors: SpotColorMapping[];
	stickerBacking: boolean;
	stickerBackingColor: string;
	stickerBleedMm: number;
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
	writeDxf: boolean;
	writeAse: boolean;
	writeSeparations: boolean;
}

export interface UiWatchConfig {
	inDir: string;
	outDir: string;
	includeSubfolders: boolean;
	params: Options;
	export: ExportOptions;
	writePdf: boolean;
	writeEps: boolean;
	writeDxf: boolean;
	writeAse: boolean;
	writeSeparations: boolean;
	pollMs?: number;
	debounceMs?: number;
}

export interface WatchEvent {
	file: string;
	fileName: string;
	status: BatchStatus;
	colors: number;
	paths: number;
	nodes: number;
	mae: number | null;
	elapsedMs: number;
	error: string | null;
}

export interface WatchStoppedEvent {
	error: string | null;
}

export type ShortcutAction =
	| "openFile"
	| "paste"
	| "copySvg"
	| "saveProject"
	| "undo"
	| "redo"
	| "toggleOutline"
	| "togglePathSelect"
	| "nodeTool"
	| "toggleSoftProof"
	| "openSettings";

export const DEFAULT_SHORTCUTS: Record<ShortcutAction, string> = {
	openFile: "Ctrl+O",
	paste: "Ctrl+V",
	copySvg: "Ctrl+Shift+C",
	saveProject: "Ctrl+S",
	undo: "Ctrl+Z",
	redo: "Ctrl+Shift+Z",
	toggleOutline: "Ctrl+Y",
	togglePathSelect: "Ctrl+E",
	nodeTool: "A",
	toggleSoftProof: "Ctrl+P",
	openSettings: "Ctrl+,",
};

export interface AppSettings {
	language: "id" | "en";
	theme: ThemeChoice;
	canvasBg: CanvasBgChoice;
	openFolderAfterExport: boolean;
	defaultColorSpace: ColorSpace;
	defaultDxfUnit: DxfUnit;
	defaultDpi: 72 | 96 | 300;
	defaultDieCutOffsetMm: number;
	defaultDieCutSpotName: string;
	defaultDieCutBridge: boolean;
	preferMedianOnly: boolean;
	undoLimit: 20 | 50 | 100;
	shortcuts: Record<string, string>;
	autoCheckUpdate: boolean;
}

export const DEFAULT_APP_SETTINGS: AppSettings = {
	language: "id",
	theme: "dark",
	canvasBg: "checkerDark",
	openFolderAfterExport: false,
	defaultColorSpace: "rgb",
	defaultDxfUnit: "mm",
	defaultDpi: 96,
	defaultDieCutOffsetMm: 2.0,
	defaultDieCutSpotName: "CutContour",
	defaultDieCutBridge: true,
	preferMedianOnly: false,
	undoLimit: 50,
	shortcuts: {},
	autoCheckUpdate: false,
};

export interface SavedPaletteColor {
	hex: string;
	spotName?: string | null;
}

export interface SavedPalette {
	id: string;
	name: string;
	colors: SavedPaletteColor[];
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
/** Single-file open also accepts vector .svg (1.3.0); batch stays raster-only. */
export const OPEN_EXTENSIONS = [...IMAGE_EXTENSIONS, "svg"];

export const processImage = (path: string, params: Options): Promise<Processed> => invoke<Processed>("process", { path, params });
export const renderPreviewSvg = (options: ExportOptions): Promise<string> => invoke<string>("render_preview_svg", { options });
export const loadRawImagePreview = (path: string): Promise<RawImagePreview> => invoke<RawImagePreview>("load_raw_image_preview", { path });
export const pickColor = (x: number, y: number): Promise<string> => invoke<string>("pick_color", { x, y });

export type ExportKind = "svg" | "pdf" | "eps" | "dxf" | "ase";
export type PaletteTextExportKind = "gpl" | "json" | "css";
export const saveFile = (kind: ExportKind, path: string, options: ExportOptions): Promise<void> => invoke<void>("save_file", { kind, path, options });
export const saveTextFile = (path: string, content: string): Promise<void> => invoke<void>("save_text_file", { path, content });
export const exportDxf = (path: string, options: ExportOptions): Promise<void> => invoke<void>("export_dxf", { path, options });
export const pasteClipboardImage = (): Promise<string> => invoke<string>("paste_clipboard_image");
export const copySvgToClipboard = (options: ExportOptions, recolor?: RecolorSpec): Promise<number> =>
	invoke<number>("copy_svg_to_clipboard", { options, recolor: recolor ?? null });
export const saveSeparations = (dir: string, baseName: string, kind: "svg" | "pdf" | "both", options: ExportOptions): Promise<string[]> =>
	invoke<string[]>("save_separations", { dir, baseName, kind, options });

export const getAppSettings = (): Promise<AppSettings> => invoke<AppSettings>("get_app_settings");
export const saveAppSettings = (settings: AppSettings): Promise<AppSettings> => invoke<AppSettings>("save_app_settings", { settings });
export const getSettingsPath = (): Promise<string> => invoke<string>("get_settings_path");

export const listSavedPalettes = (): Promise<SavedPalette[]> => invoke<SavedPalette[]>("list_saved_palettes");
export const savePaletteLibraryItem = (palette: SavedPalette): Promise<SavedPalette[]> => invoke<SavedPalette[]>("save_palette_library_item", { palette });
export const deleteSavedPalette = (id: string): Promise<SavedPalette[]> => invoke<SavedPalette[]>("delete_saved_palette", { id });


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
export const startWatchFolder = (config: UiWatchConfig): Promise<void> => invoke<void>("start_watch_folder", { config });
export const stopWatchFolder = (): Promise<void> => invoke<void>("stop_watch_folder");
export const openFolder = (dir: string): Promise<void> => invoke<void>("open_folder", { dir });

export const inspectPathNodes = (targetId: string): Promise<PathNodeGeometry | null> => invoke<PathNodeGeometry | null>("inspect_path_nodes", { targetId });
export const sidecarAvailable = (): Promise<boolean> => invoke<boolean>("sidecar_available");
export const licenses = (): Promise<string> => invoke<string>("third_party_licenses");

export async function pickImageOrProject(): Promise<string | null> {
	const r = await open({
		multiple: false,
		directory: false,
		filters: [
			{ name: "Images, SVG & Trasea Projects", extensions: [...OPEN_EXTENSIONS, "trasea"] },
			{ name: "Images", extensions: IMAGE_EXTENSIONS },
			{ name: "SVG", extensions: ["svg"] },
			{ name: "Trasea Project", extensions: ["trasea"] },
		],
	});
	return typeof r === "string" ? r : null;
}

export async function pickImage(): Promise<string | null> {
	const r = await open({ multiple: false, directory: false, filters: [{ name: "Images & SVG", extensions: OPEN_EXTENSIONS }] });
	return typeof r === "string" ? r : null;
}

export async function pickImages(): Promise<string[]> {
	const r = await open({ multiple: true, directory: false, filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }] });
	if (Array.isArray(r)) return r.filter((x): x is string => typeof x === "string");
	return typeof r === "string" ? [r] : [];
}

export async function pickFolder(title?: string): Promise<string | null> {
	const r = await open({ multiple: false, directory: true, title });
	return typeof r === "string" ? r : null;
}

export async function pickSavePath(kind: ExportKind, suggested: string): Promise<string | null> {
	const label =
		kind === "ase"
			? "Adobe Swatch Exchange"
			: kind === "eps"
				? "Encapsulated PostScript"
				: kind === "dxf"
					? "AutoCAD DXF R12"
					: kind.toUpperCase();
	return save({ defaultPath: suggested, filters: [{ name: label, extensions: [kind] }] });
}

export async function pickPaletteTextSavePath(kind: PaletteTextExportKind, suggested: string): Promise<string | null> {
	const label = kind === "gpl" ? "GIMP / Inkscape Palette (.gpl)" : kind === "css" ? "CSS Variables (.css)" : "JSON Palette (.json)";
	return save({ defaultPath: suggested, filters: [{ name: label, extensions: [kind] }] });
}

export async function pickProjectSavePath(suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: "Trasea Project", extensions: ["trasea"] }] });
}

export const onProgress = (fn: (stage: string, fraction: number) => void) => listen<{ stage: string; fraction: number }>("progress", e => fn(e.payload.stage, e.payload.fraction));
export const onBatchProgress = (fn: (prog: BatchProgress) => void) => listen<BatchProgress>("batch-progress", e => fn(e.payload));
export const onWatchEvent = (fn: (ev: WatchEvent) => void) => listen<WatchEvent>("watch-event", e => fn(e.payload));
export const onWatchStopped = (fn: (ev: WatchStoppedEvent) => void) => listen<WatchStoppedEvent>("watch-stopped", e => fn(e.payload));

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

export interface UpdateInfo {
	currentVersion: string;
	latestVersion: string;
	hasUpdate: boolean;
	releaseNotes: string;
	downloadUrl: string;
	isPortable: boolean;
}

export interface AppModeInfo {
	isPortable: boolean;
	currentVersion: string;
}

export const checkForUpdate = (): Promise<UpdateInfo> => invoke<UpdateInfo>("check_for_update");
export const getAppMode = (): Promise<AppModeInfo> => invoke<AppModeInfo>("get_app_mode");
export const openExternalUrl = (url: string): Promise<void> => invoke<void>("open_external_url", { url });

export type AiProvider = "local" | "recraft" | "vectorizer_ai" | "claude" | "openai" | "gemini" | "cli";

export interface AiConfigPublic {
	aiProvider: string;
	aiClaudeModel: string;
	aiOpenaiModel: string;
	aiGeminiModel: string;
	aiCliExecutable: string;
	aiCliArgs: string[];
	aiCliTimeoutSec: number;
	aiCliModel: string;
	aiCliReasoningEffort: string;
	hasRecraftKey: boolean;
	hasVectorizerKeys: boolean;
	hasClaudeKey: boolean;
	hasOpenaiKey: boolean;
	hasGeminiKey: boolean;
}

export interface SaveAiConfigInput {
	aiProvider?: string;
	aiClaudeModel?: string;
	aiOpenaiModel?: string;
	aiGeminiModel?: string;
	aiCliExecutable?: string;
	aiCliArgs?: string[];
	aiCliTimeoutSec?: number;
	aiCliModel?: string;
	aiCliReasoningEffort?: string;
	recraftApiKey?: string;
	vectorizerApiId?: string;
	vectorizerApiSecret?: string;
	claudeApiKey?: string;
	openaiApiKey?: string;
	geminiApiKey?: string;
}

export interface AiModelItem {
	id: string;
	label: string;
}

export interface FetchAiModelsResult {
	provider: string;
	status: string;
	message: string;
	models: AiModelItem[];
}

export interface AiTestResult {
	ok: boolean;
	provider: string;
	message: string;
	resolvedPath: string | null;
}

export const getAiConfig = (): Promise<AiConfigPublic> => invoke<AiConfigPublic>("get_ai_config");
export const saveAiConfig = (input: SaveAiConfigInput): Promise<AiConfigPublic> =>
	invoke<AiConfigPublic>("save_ai_config", { input });
export const fetchAiModels = (provider: string): Promise<FetchAiModelsResult> =>
	invoke<FetchAiModelsResult>("fetch_ai_models", { provider });
export const testAiConfig = (provider?: string): Promise<AiTestResult> =>
	invoke<AiTestResult>("test_ai_config", { provider: provider ?? null });
export const vectorizeWithAi = (path: string, params: Options, provider?: string): Promise<Processed> =>
	invoke<Processed>("vectorize_with_ai", { path, params, provider: provider ?? null });
export const fetchCliModels = (executable: string): Promise<FetchAiModelsResult> =>
	invoke<FetchAiModelsResult>("fetch_cli_models", { executable });
export const applyAiVectorEdits = (path: string, params: Options): Promise<Processed> =>
	invoke<Processed>("apply_ai_vector_edits", { path, params });

// --- v1.8.0: Pre-Flight Audit, Smart Recolor, Brand Kit ZIP, Compare Engines, Mockup ---
export type RecolorMode = "original" | "monoBlack" | "monoWhite" | "duotone";

export interface RecolorSpec {
	mode: RecolorMode;
	primary?: string | null;
	secondary?: string | null;
}

export interface AuditIssue {
	code: string;
	severity: "warn" | "info" | string;
	count: number;
	detail: string;
	fixable: boolean;
}

export interface AuditReport {
	ok: boolean;
	totalPaths: number;
	totalNodes: number;
	totalColors: number;
	issues: AuditIssue[];
}

export interface AutoFixResult {
	fixes: number;
	svg: string;
	paths: number;
	nodes: number;
	bytes: number;
	palette: string[];
	report: AuditReport;
}

export interface SwitchSourceResult {
	source: "offline" | "ai";
	svg: string;
	paths: number;
	nodes: number;
	bytes: number;
	palette: string[];
	report: AuditReport;
}

export const auditActiveVector = (): Promise<AuditReport> =>
	invoke<AuditReport>("audit_active_vector");

export const autoFixActiveVector = (options: ExportOptions): Promise<AutoFixResult> =>
	invoke<AutoFixResult>("auto_fix_active_vector", { options });

export const exportBrandKitZip = (path: string, baseName: string, options: ExportOptions): Promise<number> =>
	invoke<number>("export_brand_kit_zip", { path, baseName, options });

export const setActiveVectorSource = (source: "offline" | "ai", options: ExportOptions): Promise<SwitchSourceResult> =>
	invoke<SwitchSourceResult>("set_active_vector_source", { source, options });

export const saveMockupPng = (path: string, pngBase64: string): Promise<number> =>
	invoke<number>("save_mockup_png", { path, pngBase64 });

export async function pickBrandKitZipSavePath(suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: "Brand Kit ZIP", extensions: ["zip"] }] });
}

export async function pickMockupPngSavePath(suggested: string): Promise<string | null> {
	return save({ defaultPath: suggested, filters: [{ name: "PNG Image", extensions: ["png"] }] });
}
