import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import BatchView from "./BatchView";
import CropModal from "./CropModal";
import Licenses from "./Licenses";
import {
	collectDroppedImages,
	copySvgToClipboard,
	DEFAULT_PREPROCESS,
	deletePreset,
	exportDxf,
	getAppSettings,
	getLicenseStatus,
	getSampleImagePath,
	licenses,
	listPresets,
	loadProjectFile,
	onDrop,
	onProgress,
	pasteClipboardImage,
	pickColor,
	pickFolder,
	pickImage,
	pickImageOrProject,
	pickProjectSavePath,
	pickSavePath,
	processImage,
	saveAppSettings,
	saveFile,
	savePreset,
	saveProjectFile,
	saveSeparations,
	sidecarAvailable,
	type BrushStroke,
	type ColorSpace,
	type DxfUnit,
	type ExportKind,
	type ExportOptions,
	type LicenseStatus,
	type Options,
	type PreprocessOptions,
	type Processed,
	type ProductionMode,
	type UserPreset,
} from "./api";
import { emptyHistory, pushSnapshot, stepRedo, stepUndo, type EditorSnapshot, type HistoryState, type TargetChoice } from "./history";
import { I18nProvider, useI18n, type Lang } from "./i18n";

const DEFAULTS: Options = {
	colors: 10,
	smooth: 50,
	keepLines: true,
	aiClean: false,
	detectPrimitives: true,
	symmetry: "off",
	detectText: true,
	customPalette: null,
	paletteMerges: [],
	targetNodes: null,
	brushStrokes: [],
	productionMode: "standard",
	minStrokePt: 0,
	stripBackground: false,
	registrationMarks: false,
	sablonKnockout: false,
	preprocess: { ...DEFAULT_PREPROCESS },
};

const DEFAULT_EXP: ExportOptions = {
	linesAs: "stroke",
	trapPt: 0,
	registrationMarks: false,
	stripBackground: false,
	colorSpace: "rgb",
	dxfUnit: "mm",
	dxfTargetWidthMm: null,
};

type View = "editor" | "batch";
type Tool = "pan" | "detail" | "smooth" | "pipet";

const GARIS_LAYER = "__GARIS__";
const REG_LAYER = "__REGISTRASI__";

const baseName = (p: string): string => (p.split(/[\\/]/).pop() ?? "gambar").replace(/\.[^.]+$/, "");
const fmtKb = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
const rgb = (hex: string): [number, number, number] => [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
const heatColor = (r: number): string => `hsl(${Math.round(120 * (1 - Math.min(1, Math.max(0, r))))}, 90%, 50%)`;

const normalizeOptions = (po: Partial<Options>): Options => ({
	...DEFAULTS,
	...po,
	sablonKnockout: po.sablonKnockout ?? false,
	preprocess: po.preprocess ? { crop: po.preprocess.crop ?? null, rotateDeg: po.preprocess.rotateDeg ?? 0 } : { ...DEFAULT_PREPROCESS },
});

const normalizeExport = (pe: Partial<ExportOptions>): ExportOptions => ({
	...DEFAULT_EXP,
	...pe,
	colorSpace: pe.colorSpace ?? "rgb",
	dxfUnit: pe.dxfUnit ?? "mm",
	dxfTargetWidthMm: pe.dxfTargetWidthMm ?? null,
});

function buildPreviewSvg(rawSvg: string, outline: boolean, hidden: Set<string>): string {
	if (!outline && hidden.size === 0) return rawSvg;
	const rules: string[] = [];
	if (outline) {
		rules.push(
			"path, circle, ellipse, rect, polygon, polyline { fill: none !important; stroke: #2563eb !important; stroke-width: 1.25px !important; vector-effect: non-scaling-stroke !important; }",
			"#Garis path { stroke: #0ea5e9 !important; }",
			"#Registrasi circle, #Registrasi line { stroke: #ef4444 !important; }",
		);
	}
	for (const key of hidden) {
		if (key === GARIS_LAYER) {
			rules.push("#Garis { display: none !important; }");
		} else if (key === REG_LAYER) {
			rules.push("#Registrasi { display: none !important; }");
		} else if (/^#[0-9A-Fa-f]{6}$/.test(key)) {
			const suffix = key.slice(1).toUpperCase();
			rules.push(`g[id$="_${suffix}"] { display: none !important; }`);
		}
	}
	if (rules.length === 0) return rawSvg;
	const styleTag = `<style>${rules.join(" ")}</style>`;
	const idx = rawSvg.indexOf(">", rawSvg.indexOf("<svg"));
	if (idx === -1) return rawSvg;
	return rawSvg.slice(0, idx + 1) + styleTag + rawSvg.slice(idx + 1);
}

function EyeIcon({ hidden }: { hidden: boolean }) {
	return hidden ? (
		<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
			<path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
			<path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
			<line x1="1" y1="1" x2="23" y2="23" />
		</svg>
	) : (
		<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
			<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
			<circle cx="12" cy="12" r="3" />
		</svg>
	);
}

function TraseaApp() {
	const { t, lang, setLang } = useI18n();
	const errText = useCallback((e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : t.unknownError), [t.unknownError]);

	const [view, setView] = useState<View>("editor");
	const [batchInitialFiles, setBatchInitialFiles] = useState<string[]>([]);
	const [path, setPath] = useState<string | null>(null);
	const [projectPath, setProjectPath] = useState<string | null>(null);
	const [missingSourceProject, setMissingSourceProject] = useState<string | null>(null);
	const [opts, setOpts] = useState<Options>(DEFAULTS);
	const [result, setResult] = useState<Processed | null>(null);
	const [busy, setBusy] = useState(false);
	const [stage, setStage] = useState("");
	const [fraction, setFraction] = useState(0);
	const [error, setError] = useState("");
	const [info, setInfo] = useState("");
	const [over, setOver] = useState(false);
	const [split, setSplit] = useState(50);
	const [exp, setExp] = useState<ExportOptions>(DEFAULT_EXP);
	const [dxfWidthInput, setDxfWidthInput] = useState("");
	const [withAse, setWithAse] = useState(true);
	const [sepPdf, setSepPdf] = useState(false);
	const [checker, setChecker] = useState(false);
	const [outlineView, setOutlineView] = useState(false);
	const [hiddenLayers, setHiddenLayers] = useState<Set<string>>(() => new Set());
	const [cropOpen, setCropOpen] = useState(false);
	const [showText, setShowText] = useState(false);
	const [showHeat, setShowHeat] = useState(false);
	const [tool, setTool] = useState<Tool>("pan");
	const [brushSize, setBrushSize] = useState(40);
	const [strokes, setStrokes] = useState<BrushStroke[]>([]);
	const [live, setLive] = useState<BrushStroke | null>(null);
	const [palette, setPalette] = useState<string[] | null>(null);
	const [paletteMerges, setPaletteMerges] = useState<Array<[string, string]>>([]);
	const [mergeSource, setMergeSource] = useState<string | null>(null);
	const [targetChoice, setTargetChoice] = useState<TargetChoice>("auto");
	const [customTarget, setCustomTarget] = useState("800");
	const [hasSidecar, setHasSidecar] = useState(true);
	const [license, setLicense] = useState<LicenseStatus>({ active: false, info: null });
	const [modalOpen, setModalOpen] = useState<{ markdown: string; tab: "license" | "privacy" | "thirdParty"; hint: string } | null>(null);
	const [presets, setPresets] = useState<UserPreset[]>([]);
	const [selectedPresetId, setSelectedPresetId] = useState("");
	const [presetNameInput, setPresetNameInput] = useState("");
	const [addingPreset, setAddingPreset] = useState(false);
	const [history, setHistory] = useState<HistoryState>(emptyHistory);
	const [appliedSig, setAppliedSig] = useState("");

	const busyRef = useRef(false);
	const optsRef = useRef(opts);
	optsRef.current = opts;
	const standardBackupRef = useRef({ colors: DEFAULTS.colors, smooth: DEFAULTS.smooth });
	const overlayRef = useRef<SVGSVGElement | null>(null);
	const liveRef = useRef<BrushStroke | null>(null);

	const targetNodes = useMemo((): number | null => {
		if (targetChoice === "auto") return null;
		const n = targetChoice === "custom" ? Number.parseInt(customTarget, 10) : Number.parseInt(targetChoice, 10);
		return Number.isFinite(n) && n > 0 ? n : null;
	}, [targetChoice, customTarget]);

	const makeSnapshot = useCallback(
		(label: string): EditorSnapshot => ({
			opts: {
				...opts,
				preprocess: { crop: opts.preprocess.crop ? [...opts.preprocess.crop] : null, rotateDeg: opts.preprocess.rotateDeg },
			},
			palette: palette ? [...palette] : null,
			paletteMerges: paletteMerges.map(m => [m[0], m[1]]),
			strokes: strokes.map(s => ({ ...s, points: s.points.map(p => [p[0], p[1]]) })),
			targetChoice,
			customTarget,
			exp: { ...exp },
			label,
		}),
		[opts, palette, paletteMerges, strokes, targetChoice, customTarget, exp],
	);

	const recordHistory = useCallback(
		(label: string) => {
			setHistory(h => pushSnapshot(h, makeSnapshot(label)));
		},
		[makeSnapshot],
	);

	const restoreSnapshot = useCallback((snap: EditorSnapshot) => {
		const normOpts = normalizeOptions(snap.opts);
		const normExp = normalizeExport(snap.exp);
		setOpts(normOpts);
		setPalette(snap.palette);
		setPaletteMerges(snap.paletteMerges);
		setStrokes(snap.strokes);
		setTargetChoice(snap.targetChoice);
		setCustomTarget(snap.customTarget);
		setExp(normExp);
		setDxfWidthInput(normExp.dxfTargetWidthMm ? String(normExp.dxfTargetWidthMm) : "");
		setChecker(normOpts.productionMode === "cutEmbroidery" && normOpts.stripBackground);
		setMergeSource(null);
	}, []);

	const doUndo = useCallback(() => {
		const res = stepUndo(history, makeSnapshot(t.undo));
		if (!res) return;
		setHistory(res.history);
		restoreSnapshot(res.target);
		setInfo(`Undo: ${res.target.label}`);
	}, [history, makeSnapshot, restoreSnapshot, t.undo]);

	const doRedo = useCallback(() => {
		const res = stepRedo(history, makeSnapshot(t.redo));
		if (!res) return;
		setHistory(res.history);
		restoreSnapshot(res.target);
		setInfo(`Redo: ${res.target.label}`);
	}, [history, makeSnapshot, restoreSnapshot, t.redo]);

	const signature = useMemo(
		() => JSON.stringify({ ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes }),
		[opts, palette, paletteMerges, targetNodes, strokes],
	);

	const openPrivacyModal = useCallback(async (tab: "license" | "privacy" | "thirdParty" = "license", hint = "") => {
		const md = await licenses();
		setModalOpen({ markdown: md, tab, hint });
	}, []);

	const effectiveExport = useCallback((): ExportOptions => {
		const parsedW = Number.parseFloat(dxfWidthInput);
		const dxfTargetWidthMm = exp.dxfUnit === "mm" && Number.isFinite(parsedW) && parsedW > 0 ? parsedW : null;
		const base: ExportOptions = { ...exp, dxfTargetWidthMm };
		if (opts.productionMode === "cutEmbroidery") {
			return { ...base, linesAs: "shape", trapPt: 0, registrationMarks: false, stripBackground: opts.stripBackground };
		}
		if (opts.productionMode === "screenPrint") {
			return { ...base, registrationMarks: opts.registrationMarks, stripBackground: false };
		}
		return { ...base, registrationMarks: false, stripBackground: false };
	}, [exp, dxfWidthInput, opts.productionMode, opts.stripBackground, opts.registrationMarks]);

	const run = useCallback(
		async (p: string, o: Options) => {
			if (busyRef.current) return;
			busyRef.current = true;
			setBusy(true);
			setError("");
			setInfo("");
			setStage(t.loadingImage);
			setFraction(0);
			setPath(p);
			try {
				const r = await processImage(p, o);
				setResult(r);
				setHiddenLayers(new Set());
				setAppliedSig(JSON.stringify(o));
			} catch (e) {
				setError(errText(e));
			} finally {
				busyRef.current = false;
				setBusy(false);
			}
		},
		[errText, t.loadingImage],
	);

	const openPicture = useCallback(
		(p: string) => {
			setView("editor");
			setProjectPath(null);
			setMissingSourceProject(null);
			setStrokes([]);
			setLive(null);
			setPalette(null);
			setPaletteMerges([]);
			setMergeSource(null);
			setHiddenLayers(new Set());
			setHistory(emptyHistory());
			setTool("pan");
			const nextOpts: Options = {
				...optsRef.current,
				customPalette: null,
				paletteMerges: [],
				brushStrokes: [],
				preprocess: { ...DEFAULT_PREPROCESS },
			};
			setOpts(nextOpts);
			void run(p, nextOpts);
		},
		[run],
	);

	const openProject = useCallback(
		async (projFile: string) => {
			setError("");
			setInfo("");
			try {
				const loaded = await loadProjectFile(projFile);
				const po = normalizeOptions(loaded.project.options);
				const pe = normalizeExport(loaded.project.exportOptions);
				setView("editor");
				setProjectPath(projFile);
				setOpts(po);
				setExp(pe);
				setDxfWidthInput(pe.dxfTargetWidthMm ? String(pe.dxfTargetWidthMm) : "");
				setPalette(po.customPalette);
				setPaletteMerges(po.paletteMerges ?? []);
				setStrokes(po.brushStrokes ?? []);
				setChecker(po.productionMode === "cutEmbroidery" && po.stripBackground);
				setHiddenLayers(new Set());
				setHistory(emptyHistory());
				if (po.targetNodes === null) setTargetChoice("auto");
				else if (["500", "1000", "2000"].includes(String(po.targetNodes))) setTargetChoice(String(po.targetNodes) as TargetChoice);
				else {
					setTargetChoice("custom");
					setCustomTarget(String(po.targetNodes));
				}

				if (loaded.sourceExists) {
					setMissingSourceProject(null);
					await run(loaded.project.sourcePath, po);
					setInfo(!loaded.hashMatches ? t.projectOpenedChanged : `${t.projectOpened} ${projFile}`);
				} else {
					setPath(null);
					setResult(null);
					setMissingSourceProject(loaded.project.sourcePath);
					setInfo(t.projectMissingSource);
				}
			} catch (e) {
				setError(errText(e));
			}
		},
		[run, errText, t.projectOpenedChanged, t.projectOpened, t.projectMissingSource],
	);

	const relinkMissingSource = async () => {
		const newImg = await pickImage();
		if (!newImg) return;
		setMissingSourceProject(null);
		const mergedOpts: Options = { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes };
		await run(newImg, mergedOpts);
		setInfo(`${t.projectRelinked} ${newImg}`);
	};

	const saveProject = useCallback(
		async (saveAs: boolean) => {
			if (!path) return;
			setError("");
			setInfo("");
			try {
				const dest = !saveAs && projectPath ? projectPath : await pickProjectSavePath(`${baseName(path)}.trasea`);
				if (!dest) return;
				const fullOpts: Options = { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes };
				await saveProjectFile(dest, path, fullOpts, effectiveExport());
				setProjectPath(dest);
				setInfo(`${t.projectSaved} ${dest}`);
			} catch (e) {
				setError(errText(e));
			}
		},
		[path, projectPath, opts, palette, paletteMerges, targetNodes, strokes, effectiveExport, errText, t.projectSaved],
	);

	const openSample = async () => {
		setError("");
		try {
			const sp = await getSampleImagePath();
			openPicture(sp);
		} catch (e) {
			setError(errText(e));
		}
	};

	const doPasteClipboard = useCallback(async () => {
		if (busyRef.current) return;
		setError("");
		setInfo("");
		try {
			const clipPath = await pasteClipboardImage();
			openPicture(clipPath);
			setInfo(t.pastedClipboardMsg);
		} catch (e) {
			setError(errText(e));
		}
	}, [openPicture, errText, t.pastedClipboardMsg]);

	const doCopySvg = useCallback(async () => {
		if (!result || busyRef.current) return;
		setError("");
		setInfo("");
		try {
			const bytes = await copySvgToClipboard(effectiveExport());
			setInfo(`${t.copiedSvgMsg} (${fmtKb(bytes)}).`);
		} catch (e) {
			setError(errText(e));
		}
	}, [result, effectiveExport, errText, t.copiedSvgMsg]);

	const choose = useCallback(async () => {
		const p = await pickImageOrProject();
		if (!p) return;
		if (p.toLowerCase().endsWith(".trasea")) void openProject(p);
		else openPicture(p);
	}, [openPicture, openProject]);

	useEffect(() => {
		void sidecarAvailable().then(setHasSidecar);
		void getLicenseStatus().then(setLicense);
		void listPresets().then(setPresets).catch(() => {});
		const subs: Array<Promise<() => void>> = [
			onProgress((s, f) => {
				setStage(s);
				setFraction(f);
			}),
			onDrop(paths => {
				const proj = paths.find(x => x.toLowerCase().endsWith(".trasea"));
				if (proj && paths.length === 1) {
					void openProject(proj);
					return;
				}
				void collectDroppedImages(paths, false).then(imgs => {
					if (imgs.length === 0) {
						setError(t.dropInvalidErr);
					} else if (imgs.length === 1 && paths.length === 1 && imgs[0] && imgs[0] === paths[0]) {
						openPicture(imgs[0]);
					} else {
						setBatchInitialFiles(imgs);
						setView("batch");
					}
				});
			}, setOver),
		];
		return () => {
			subs.forEach(s => void s.then(u => u()));
		};
	}, [openPicture, openProject, t.dropInvalidErr]);

	// Global keyboard shortcuts:
	// Ctrl+Z (Undo), Ctrl+Shift+Z (Redo), Ctrl+Y (Outline toggle), Ctrl+V (Paste image),
	// Ctrl+Shift+C (Copy SVG), Ctrl+S / Ctrl+Shift+S (Save project), Ctrl+O (Open)
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			const tag = (e.target as HTMLElement | null)?.tagName ?? "";
			const editingText = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
			if (!e.ctrlKey && !e.metaKey) return;
			const k = e.key.toLowerCase();
			if (k === "z" && !editingText && view === "editor") {
				e.preventDefault();
				if (e.shiftKey) doRedo();
				else doUndo();
			} else if (k === "y" && !editingText && view === "editor" && result) {
				e.preventDefault();
				setOutlineView(v => !v);
			} else if (k === "v" && !editingText && view === "editor" && !modalOpen && !cropOpen) {
				e.preventDefault();
				void doPasteClipboard();
			} else if (k === "c" && e.shiftKey && !editingText && view === "editor" && result) {
				e.preventDefault();
				void doCopySvg();
			} else if (k === "s" && view === "editor" && path) {
				e.preventDefault();
				void saveProject(e.shiftKey);
			} else if (k === "o" && !busyRef.current) {
				e.preventDefault();
				void choose();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	}, [view, path, result, modalOpen, cropOpen, doUndo, doRedo, doPasteClipboard, doCopySvg, saveProject, choose]);

	const switchProductionMode = (mode: ProductionMode) => {
		const mName = mode === "screenPrint" ? t.modeScreenPrint : mode === "cutEmbroidery" ? t.modeCutEmbroidery : mode === "shading" ? t.modeShading : t.modeStandard;
		recordHistory(`${t.productionModeTitle}: ${mName}`);
		setSelectedPresetId("");
		setOpts(o => {
			if (o.productionMode === "standard" && mode !== "standard") {
				standardBackupRef.current = { colors: o.colors, smooth: o.smooth };
			}
			if (mode === "standard") {
				return {
					...o,
					productionMode: "standard",
					colors: o.productionMode === "standard" ? o.colors : standardBackupRef.current.colors,
					smooth: o.productionMode === "standard" ? o.smooth : standardBackupRef.current.smooth,
					minStrokePt: 0,
					stripBackground: false,
					registrationMarks: false,
					sablonKnockout: false,
				};
			}
			if (mode === "screenPrint") {
				return {
					...o,
					productionMode: "screenPrint",
					colors: o.productionMode === "screenPrint" ? Math.min(8, Math.max(2, o.colors)) : 4,
					smooth: o.productionMode === "cutEmbroidery" ? standardBackupRef.current.smooth : o.smooth,
					minStrokePt: o.productionMode === "screenPrint" ? o.minStrokePt : 1.0,
					stripBackground: false,
					registrationMarks: true,
				};
			}
			if (mode === "shading") {
				return {
					...o,
					productionMode: "shading",
					colors: o.productionMode === "shading" ? Math.min(32, Math.max(12, o.colors)) : 18,
					smooth: o.productionMode === "shading" ? o.smooth : 50,
					minStrokePt: 0,
					stripBackground: false,
					registrationMarks: false,
					sablonKnockout: false,
				};
			}
			return {
				...o,
				productionMode: "cutEmbroidery",
				colors: o.productionMode === "screenPrint" || o.productionMode === "shading" ? standardBackupRef.current.colors : o.colors,
				smooth: o.productionMode === "cutEmbroidery" ? o.smooth : 70,
				minStrokePt: o.productionMode === "cutEmbroidery" ? o.minStrokePt : 1.5,
				stripBackground: true,
				registrationMarks: false,
				sablonKnockout: false,
			};
		});
		if (mode === "standard" || mode === "shading") {
			setExp(ex => ({ ...ex, linesAs: "stroke", trapPt: 0, registrationMarks: false, stripBackground: false }));
			setChecker(false);
		} else if (mode === "screenPrint") {
			setExp(ex => ({ ...ex, linesAs: "stroke", trapPt: 0.5, registrationMarks: true, stripBackground: false }));
			setChecker(false);
		} else {
			setExp(ex => ({ ...ex, linesAs: "shape", trapPt: 0, registrationMarks: false, stripBackground: true }));
			setChecker(true);
		}
	};

	const applyPreset = (preset: UserPreset) => {
		recordHistory(`${t.presetAppliedPrefix} "${preset.name}"`);
		setSelectedPresetId(preset.id);
		const po = normalizeOptions(preset.options);
		const pe = normalizeExport(preset.exportOptions);
		if (po.productionMode === "standard") {
			standardBackupRef.current = { colors: po.colors, smooth: po.smooth };
		}
		setOpts({ ...po, customPalette: palette, paletteMerges, brushStrokes: strokes, preprocess: opts.preprocess });
		setExp(pe);
		setDxfWidthInput(pe.dxfTargetWidthMm ? String(pe.dxfTargetWidthMm) : "");
		setChecker(po.productionMode === "cutEmbroidery" && po.stripBackground);
		if (po.targetNodes === null) setTargetChoice("auto");
		else if (["500", "1000", "2000"].includes(String(po.targetNodes))) setTargetChoice(String(po.targetNodes) as TargetChoice);
		else {
			setTargetChoice("custom");
			setCustomTarget(String(po.targetNodes));
		}
		setInfo(`${t.presetAppliedPrefix} "${preset.name}" ${t.presetAppliedSuffix}`);
	};

	const createPreset = async () => {
		const name = presetNameInput.trim();
		if (!name) return;
		setError("");
		try {
			const id = `preset-${Date.now()}`;
			const next = await savePreset({
				id,
				name,
				options: { ...opts, customPalette: null, paletteMerges: [], brushStrokes: [], targetNodes, preprocess: { ...DEFAULT_PREPROCESS } },
				exportOptions: effectiveExport(),
			});
			setPresets(next);
			setSelectedPresetId(id);
			setPresetNameInput("");
			setAddingPreset(false);
			setInfo(`${t.presetAppliedPrefix} "${name}" ${t.presetSavedSuffix}`);
		} catch (e) {
			setError(errText(e));
		}
	};

	const removeSelectedPreset = async () => {
		if (!selectedPresetId) return;
		try {
			const next = await deletePreset(selectedPresetId);
			setPresets(next);
			setSelectedPresetId("");
			setInfo(t.presetDeleted);
		} catch (e) {
			setError(errText(e));
		}
	};

	const resetPalette = () => {
		recordHistory(t.resetPalette);
		setPalette(null);
		setPaletteMerges([]);
		setMergeSource(null);
		if (path && !busyRef.current) void run(path, { ...opts, customPalette: null, paletteMerges: [], targetNodes, brushStrokes: strokes });
	};

	const apply = () => {
		setMergeSource(null);
		if (path) void run(path, { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes });
	};

	const applyPreprocess = (pp: PreprocessOptions) => {
		recordHistory(t.cropRotate);
		setCropOpen(false);
		const nextOpts: Options = { ...opts, preprocess: pp, customPalette: palette, paletteMerges, targetNodes, brushStrokes: [] };
		setStrokes([]);
		setOpts(nextOpts);
		if (path) void run(path, nextOpts);
	};

	const resetPreprocess = () => {
		applyPreprocess({ ...DEFAULT_PREPROCESS });
	};

	const exportAs = async (kind: ExportKind) => {
		if (!path) return;
		if (kind === "eps" && !license.active) {
			void openPrivacyModal("license", t.epsProPrompt);
			return;
		}
		setError("");
		setInfo("");
		const effExp = effectiveExport();
		try {
			const target = await pickSavePath(kind, `${baseName(path)}.${kind}`);
			if (!target) return;
			if (kind === "dxf") {
				await exportDxf(target, effExp);
			} else {
				await saveFile(kind, target, effExp);
			}
			let msg = `${t.savedLabel} ${target}`;
			if (kind !== "ase" && kind !== "dxf" && withAse) {
				const ase = target.replace(/\.[^.\\/]+$/, "") + ".ase";
				await saveFile("ase", ase, effExp);
				msg += ` ${t.andLabel} ${ase}`;
			}
			setInfo(msg);
		} catch (e) {
			setError(errText(e));
		}
	};

	const exportSeparationFilms = async () => {
		if (!path) return;
		if (!license.active) {
			void openPrivacyModal("license", t.sepProPrompt);
			return;
		}
		setError("");
		setInfo("");
		try {
			const dir = await pickFolder(t.chooseSepFolderTitle);
			if (!dir) return;
			const effExp: ExportOptions = { ...effectiveExport(), registrationMarks: true, stripBackground: false };
			const written = await saveSeparations(dir, baseName(path), sepPdf ? "both" : "svg", effExp);
			setInfo(`${t.savedSepPrefix} ${written.length} ${t.savedSepMid} ${dir}`);
		} catch (e) {
			setError(errText(e));
		}
	};

	const set = (patch: Partial<Options>, label = t.settingsTitle) => {
		recordHistory(label);
		setSelectedPresetId("");
		setOpts(o => {
			const next = { ...o, ...patch };
			if (next.productionMode === "standard") {
				standardBackupRef.current = { colors: next.colors, smooth: next.smooth };
			}
			return next;
		});
	};

	const previewSvg = useMemo(
		() => (result ? buildPreviewSvg(result.svg, outlineView, hiddenLayers) : ""),
		[result, outlineView, hiddenLayers],
	);
	const svgUrl = useMemo(() => (previewSvg ? URL.createObjectURL(new Blob([previewSvg], { type: "image/svg+xml" })) : ""), [previewSvg]);
	useEffect(() => () => { if (svgUrl) URL.revokeObjectURL(svgUrl); }, [svgUrl]);

	const toggleLayer = (key: string) => {
		setHiddenLayers(prev => {
			const next = new Set(prev);
			if (next.has(key)) next.delete(key);
			else next.add(key);
			return next;
		});
	};

	const currentPalette = (): string[] => palette ?? result?.palette ?? [];
	const removeColor = (hex: string) => {
		const base = currentPalette();
		if (base.length <= 2) return;
		recordHistory(`${t.removeBtn} ${hex}`);
		setPalette(base.filter(c => c !== hex));
		setMergeSource(null);
	};
	const othersByDistance = (hex: string): string[] => {
		const a = rgb(hex);
		return currentPalette()
			.filter(c => c !== hex)
			.sort((x, y) => {
				const bx = rgb(x);
				const by = rgb(y);
				const dx = (a[0] - bx[0]) ** 2 + (a[1] - bx[1]) ** 2 + (a[2] - bx[2]) ** 2;
				const dy = (a[0] - by[0]) ** 2 + (a[1] - by[1]) ** 2 + (a[2] - by[2]) ** 2;
				return dx - dy;
			});
	};
	const mergeColorInto = (fromHex: string, targetHex: string) => {
		const base = currentPalette();
		if (base.length <= 2 || fromHex === targetHex) return;
		recordHistory(`${t.mergeBtn} ${fromHex} → ${targetHex}`);
		setPalette(base.filter(c => c !== fromHex));
		setPaletteMerges(prev => [...prev.map(([a, b]): [string, string] => [a, b === fromHex ? targetHex : b]), [fromHex, targetHex]]);
		setMergeSource(null);
		setInfo(`${t.colorExistsPrefix} ${fromHex} ${t.colorMergedMid} ${targetHex}. ${t.colorMergedSuffix}`);
	};
	const addColor = (hex: string) => {
		const base = currentPalette();
		if (base.includes(hex)) {
			setInfo(`${t.colorExistsPrefix} ${hex} ${t.colorExistsSuffix}`);
			return;
		}
		recordHistory(`${t.toolPipet} ${hex}`);
		setPalette([...base, hex]);
		setInfo(`${t.colorExistsPrefix} ${hex} ${t.colorAddedSuffix}`);
	};

	const toImage = (e: ReactPointerEvent<SVGSVGElement>): [number, number] => {
		const r = (overlayRef.current as SVGSVGElement).getBoundingClientRect();
		return [((e.clientX - r.left) / r.width) * (result?.width ?? 1), ((e.clientY - r.top) / r.height) * (result?.height ?? 1)];
	};
	const onDown = (e: ReactPointerEvent<SVGSVGElement>) => {
		if (!result || busy || e.button !== 0) return;
		const [x, y] = toImage(e);
		if (tool === "pipet") {
			void pickColor(Math.round(x), Math.round(y)).then(addColor).catch(err => setError(errText(err)));
			return;
		}
		if (tool === "detail" || tool === "smooth") {
			e.currentTarget.setPointerCapture(e.pointerId);
			const s: BrushStroke = { kind: tool, radius: brushSize, points: [[x, y]] };
			liveRef.current = s;
			setLive(s);
		}
	};
	const onMove = (e: ReactPointerEvent<SVGSVGElement>) => {
		const s = liveRef.current;
		if (!s) return;
		const [x, y] = toImage(e);
		const last = s.points[s.points.length - 1];
		if (!last) return;
		if (Math.hypot(x - last[0], y - last[1]) < 2) return;
		const next: BrushStroke = { ...s, points: [...s.points, [x, y]] };
		liveRef.current = next;
		setLive(next);
	};
	const onUp = () => {
		const s = liveRef.current;
		liveRef.current = null;
		setLive(null);
		if (s) {
			recordHistory(s.kind === "detail" ? t.toolDetail : t.toolSmooth);
			setStrokes(list => [...list, s]);
		}
	};

	const hasPreprocess = opts.preprocess.crop !== null || Math.abs(opts.preprocess.rotateDeg) > 0.05;
	const hasRegLayer = result !== null && result.svg.includes('id="Registrasi"');
	const dirty = result !== null && appliedSig !== "" && signature !== appliedSig;
	const paletteDirty = result !== null && palette !== null && JSON.stringify(palette) !== JSON.stringify(result.palette);
	const painting = tool === "detail" || tool === "smooth";
	const shown = live ? [...strokes, live] : strokes;
	const maxColors = opts.productionMode === "screenPrint" ? 8 : opts.productionMode === "shading" ? 32 : 16;
	const minColors = opts.productionMode === "shading" ? 12 : 2;
	const undoTip = history.past.length > 0 ? `${t.undoTipPrefix}: ${history.past[history.past.length - 1]?.label ?? ""}` : t.undoTipPrefix;
	const redoTip = history.future.length > 0 ? `${t.redoTipPrefix}: ${history.future[0]?.label ?? ""}` : t.redoTipPrefix;
	const locale = lang === "en" ? "en-US" : "id-ID";

	return (
		<div className={`app ${over ? "dragging" : ""}`} data-signature={signature.length}>
			<header className="top">
				<div className="brand">
					<span className="logo" aria-hidden="true">T</span> Trasea <small>{t.headerOffline}</small>
					<button
						className={`small lic-pill ${license.active ? "lic-pro" : "lic-trial"}`}
						onClick={() => void openPrivacyModal("license")}
						title={t.licenseTitle}
					>
						{license.active ? t.proActive : t.trialActivate}
					</button>
				</div>
				<div className="row">
					<div className="seg seg2 lang-switch" role="group" aria-label={t.switchLangTip} title={t.switchLangTip}>
						<button aria-pressed={lang === "id"} onClick={() => setLang("id")}>ID</button>
						<button aria-pressed={lang === "en"} onClick={() => setLang("en")}>EN</button>
					</div>
					<button className="ghost" onClick={() => void doPasteClipboard()} disabled={busy} title={t.pasteImageTip}>{t.pasteImage}</button>
					<button aria-pressed={view === "batch"} onClick={() => setView(v => (v === "batch" ? "editor" : "batch"))} disabled={busy}>
						{t.batchFolder}
					</button>
					{view === "editor" && path && (
						<button className="ghost" onClick={() => void saveProject(false)} disabled={busy} title={t.saveProjectTip}>{t.saveProject}</button>
					)}
					<button className="ghost" onClick={() => void openPrivacyModal("license")}>{t.privacyLicense}</button>
					<button onClick={() => void choose()} disabled={busy} title={t.openImageProjectTip}>{t.openImageProject}</button>
				</div>
			</header>

			{view === "batch" ? (
				<BatchView
					opts={opts}
					exp={effectiveExport()}
					initialFiles={batchInitialFiles}
					license={license}
					presets={presets}
					onApplyPreset={applyPreset}
					onOpenLicenseModal={hint => void openPrivacyModal("license", hint ?? "")}
					onChangeMode={switchProductionMode}
					onOpenInEditor={p => openPicture(p)}
					onBack={() => setView("editor")}
				/>
			) : (
				<main className="work">
					<section className="stage" aria-label={t.stageAria}>
						{!result && !busy && (
							<div className="drop-wrap">
								{missingSourceProject && (
									<div className="warn" style={{ marginBottom: 12 }}>
										{t.missingSourcePrefix} <code>{missingSourceProject}</code>. {t.missingSourceSuffix}
										<div className="row" style={{ marginTop: 8 }}>
											<button onClick={() => void relinkMissingSource()}>{t.relinkSource}</button>
										</div>
									</div>
								)}
								<div className="drop" role="button" tabIndex={0} onClick={() => void choose()} onKeyDown={e => { if (e.key === "Enter" || e.key === " ") void choose(); }}>
									<div className="drop-title">{t.dropTitle}</div>
									<div className="drop-sub">{t.dropSub1}</div>
									<div className="drop-sub">{t.dropSub2}</div>
								</div>
								<div className="row" style={{ justifyContent: "center", marginTop: 12 }}>
									<button onClick={() => void openSample()}>{t.trySample}</button>
									<button onClick={() => void doPasteClipboard()}>{t.pasteFromClipboard}</button>
									<button onClick={() => setView("batch")}>{t.processBatchBtn}</button>
								</div>
							</div>
						)}
						{busy && (
							<div className="progress" role="status" aria-live="polite">
								<div className="bar"><div className="fill" style={{ width: `${Math.round(fraction * 100)}%` }} /></div>
								<div>{stage}… {Math.round(fraction * 100)}%</div>
								<div className="muted">{t.aiWaitHint}</div>
							</div>
						)}
						{result && !busy && (
							<div className="stagebox">
								<div className="toolbar" role="toolbar" aria-label={t.toolbarAria}>
									<div className="group" role="group" aria-label={t.modeGroupAria}>
										<button aria-pressed={tool === "pan"} onClick={() => setTool("pan")}>{t.toolPan}</button>
										<button aria-pressed={tool === "detail"} onClick={() => setTool("detail")} className="tool-detail">{t.toolDetail}</button>
										<button aria-pressed={tool === "smooth"} onClick={() => setTool("smooth")} className="tool-smooth">{t.toolSmooth}</button>
										<button aria-pressed={tool === "pipet"} onClick={() => setTool("pipet")}>{t.toolPipet}</button>
									</div>
									<div className="group" role="group" aria-label={t.historyGroupAria}>
										<button onClick={doUndo} disabled={history.past.length === 0} title={undoTip}>{t.undo}</button>
										<button onClick={doRedo} disabled={history.future.length === 0} title={redoTip}>{t.redo}</button>
									</div>
									{painting && (
										<div className="group">
											<label className="brushsize" htmlFor="brush">{t.brushSize} <b>{brushSize}</b> px</label>
											<input id="brush" type="range" min={10} max={120} value={brushSize} onChange={e => setBrushSize(Number(e.target.value))} />
											<button onClick={() => { recordHistory(t.clearStrokes); setStrokes([]); }} disabled={strokes.length === 0}>{t.clearStrokes}</button>
										</div>
									)}
									<div className="group" role="group" aria-label={t.overlayGroupAria}>
										<button aria-pressed={outlineView} onClick={() => setOutlineView(v => !v)} title={t.outlineTip}>{t.outlineView}</button>
										<button aria-pressed={hasPreprocess} onClick={() => setCropOpen(true)} title={t.cropRotateTip}>{t.cropRotate}</button>
										<button aria-pressed={showHeat} onClick={() => setShowHeat(h => !h)}>{t.heatmapNode}</button>
										<button aria-pressed={checker} onClick={() => setChecker(c => !c)}>{t.checkerBg}</button>
									</div>
								</div>
								{tool === "pipet" && <div className="hint">{t.pipetHint}</div>}
								{painting && (
									<div className="hint">
										{t.brushHintPrefix} {tool === "detail" ? t.brushHintDetail : t.brushHintSmooth}. {t.brushHintSuffix}
									</div>
								)}
								<div className={`compare ${checker ? "checker" : ""}`} style={{ aspectRatio: `${result.width} / ${result.height}` }}>
									<img className="layer" src={result.original} alt={t.tagOriginal} draggable={false} style={{ clipPath: `inset(0 ${100 - split}% 0 0)` }} />
									<img className="layer vec" src={svgUrl} alt={t.tagVector} draggable={false} style={{ clipPath: `inset(0 0 0 ${split}%)` }} />
									<div className="divider" style={{ left: `${split}%` }} aria-hidden="true" />
									{showText && result.textRegions.map((r, i) => (
										<div key={i} className="textbox" aria-hidden="true" style={{ left: `${(r.x / result.width) * 100}%`, top: `${(r.y / result.height) * 100}%`, width: `${(r.w / result.width) * 100}%`, height: `${(r.h / result.height) * 100}%` }} />
									))}
									<svg ref={overlayRef} className="overlay" viewBox={`0 0 ${result.width} ${result.height}`} preserveAspectRatio="none" aria-hidden="true" data-testid="overlay"
										style={{ pointerEvents: tool === "pan" ? "none" : "auto", cursor: tool === "pipet" ? "crosshair" : painting ? "cell" : "default" }}
										onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onPointerCancel={onUp}>
										{showHeat && (() => {
											const hm = result.heatmap;
											const cell = Math.max(result.width, result.height) / 32;
											return hm.cells.map((v, i) => v > 0 ? <rect key={i} className="heatcell" x={(i % hm.cols) * cell} y={Math.floor(i / hm.cols) * cell} width={cell} height={cell} fill={heatColor(v / Math.max(1, hm.maxCell))} fillOpacity={0.25 + 0.35 * (v / Math.max(1, hm.maxCell))} /> : null);
										})()}
										{shown.map((s, i) => !s.points[0] ? null : s.points.length === 1
											? <circle key={i} cx={s.points[0][0]} cy={s.points[0][1]} r={s.radius} fill={s.kind === "detail" ? "#3b82f6" : "#f59e0b"} fillOpacity={0.4} />
											: <polyline key={i} points={s.points.map(p => `${p[0]},${p[1]}`).join(" ")} fill="none" stroke={s.kind === "detail" ? "#3b82f6" : "#f59e0b"} strokeOpacity={0.4} strokeWidth={s.radius * 2} strokeLinecap="round" strokeLinejoin="round" />)}
									</svg>
									<span className="tag left">{t.tagOriginal}</span>
									<span className="tag right">{outlineView ? t.tagOutline : t.tagVector}</span>
									<input className="slider" type="range" min={0} max={100} value={split} onChange={e => setSplit(Number(e.target.value))} aria-label={t.compareSliderAria} style={{ pointerEvents: tool === "pan" ? "auto" : "none" }} />
								</div>
								{showHeat && (
									<div className="legend" aria-label={t.heatmapLegendAria}>
										<span>{t.heatFew}</span><span className="ramp" aria-hidden="true" /><span>{t.heatDensePrefix}{result.heatmap.maxCell}{t.heatDenseSuffix}</span>
									</div>
								)}
							</div>
						)}
					</section>

					<aside className="panel" aria-label={t.panelAria}>
						<div className="palette-head" style={{ marginTop: 0 }}>
							<h2>{t.customPresets}</h2>
							<button className="small" onClick={() => setAddingPreset(v => !v)}>{addingPreset ? t.close : t.addPresetBtn}</button>
						</div>
						{addingPreset && (
							<div className="preset-add">
								<input
									className="num"
									type="text"
									placeholder={t.presetPlaceholder}
									aria-label={t.presetNameAria}
									value={presetNameInput}
									onChange={e => setPresetNameInput(e.target.value)}
									onKeyDown={e => { if (e.key === "Enter") void createPreset(); }}
								/>
								<button className="small" onClick={() => void createPreset()} disabled={!presetNameInput.trim()}>{t.save}</button>
							</div>
						)}
						{presets.length > 0 && (
							<div className="preset-row">
								<select
									aria-label={t.selectPresetAria}
									value={selectedPresetId}
									onChange={e => {
										const p = presets.find(x => x.id === e.target.value);
										if (p) applyPreset(p);
									}}
								>
									<option value="">{t.chooseSavedPreset}</option>
									{presets.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
								</select>
								<button className="small" onClick={() => void removeSelectedPreset()} disabled={!selectedPresetId} aria-label={t.deletePresetAria}>{t.delete}</button>
							</div>
						)}

						<h2>{t.productionModeTitle}</h2>
						<div className="seg seg4-grid" role="group" aria-label={t.productionModeAria}>
							<button aria-pressed={opts.productionMode === "standard"} onClick={() => switchProductionMode("standard")}>{t.modeStandard}</button>
							<button aria-pressed={opts.productionMode === "screenPrint"} onClick={() => switchProductionMode("screenPrint")}>{t.modeScreenPrint}</button>
							<button aria-pressed={opts.productionMode === "cutEmbroidery"} onClick={() => switchProductionMode("cutEmbroidery")}>{t.modeCutEmbroidery}</button>
							<button aria-pressed={opts.productionMode === "shading"} onClick={() => switchProductionMode("shading")}>{t.modeShading}</button>
						</div>

						{opts.productionMode === "screenPrint" && (
							<div className="mode-box">
								<label>{t.sablonMethodLabel}</label>
								<div className="seg seg2" role="group" aria-label={t.sablonMethodLabel} style={{ marginTop: 4 }}>
									<button aria-pressed={!opts.sablonKnockout} onClick={() => set({ sablonKnockout: false }, t.sablonStacking)}>{t.sablonStacking}</button>
									<button aria-pressed={opts.sablonKnockout} onClick={() => set({ sablonKnockout: true }, t.sablonKnockout)}>{t.sablonKnockout}</button>
								</div>
								<label htmlFor="minStroke">{t.minStrokeLabel}</label>
								<input id="minStroke" className="num" type="number" min={0} max={6} step={0.25} value={opts.minStrokePt} onChange={e => set({ minStrokePt: Math.max(0, Number(e.target.value)) }, t.minStrokeLabel)} />
								<label className="check"><input type="checkbox" checked={opts.registrationMarks} onChange={e => { const v = e.target.checked; set({ registrationMarks: v }, t.regMarksLabel); setExp(ex => ({ ...ex, registrationMarks: v })); }} /> {t.regMarksLabel}</label>
								{opts.colors > 6 && <div className="warn">{t.sablonOver6Warn}</div>}
								<div className="hint">{opts.sablonKnockout ? t.sablonKnockoutHint : t.sablonHint}</div>
							</div>
						)}

						{opts.productionMode === "cutEmbroidery" && (
							<div className="mode-box">
								<label className="check"><input type="checkbox" checked={opts.stripBackground} onChange={e => { const v = e.target.checked; set({ stripBackground: v }, t.stripBgLabel); setExp(ex => ({ ...ex, stripBackground: v })); setChecker(v); }} /> {t.stripBgLabel}</label>
								<label htmlFor="minStrokeCut">{t.minStrokeLabel}</label>
								<input id="minStrokeCut" className="num" type="number" min={0} max={6} step={0.25} value={opts.minStrokePt} onChange={e => set({ minStrokePt: Math.max(0, Number(e.target.value)) }, t.minStrokeLabel)} />
								<div className="hint">{t.cutHint}</div>
							</div>
						)}

						{opts.productionMode === "shading" && (
							<div className="mode-box">
								<div className="hint">{t.shadingHint}</div>
							</div>
						)}

						{hasPreprocess && (
							<div className="mode-box row between" style={{ marginTop: 8 }}>
								<span className="hint" style={{ color: "var(--ink)" }}>
									{t.preprocessActive}
									{Math.abs(opts.preprocess.rotateDeg) > 0.05 ? ` (${opts.preprocess.rotateDeg}°)` : ""}
									{opts.preprocess.crop ? ` · ${opts.preprocess.crop[2]}×${opts.preprocess.crop[3]}` : ""}
								</span>
								<button className="small" onClick={resetPreprocess}>{t.preprocessReset}</button>
							</div>
						)}

						<h2>{t.settingsTitle}</h2>
						<label htmlFor="colors">{t.colorCount} <b>{Math.min(maxColors, Math.max(minColors, opts.colors))}</b></label>
						<input id="colors" type="range" min={minColors} max={maxColors} value={Math.min(maxColors, Math.max(minColors, opts.colors))} onChange={e => set({ colors: Number(e.target.value) }, t.colorCount)} />
						<label htmlFor="smooth">{t.smoothness} <b>{opts.smooth < 34 ? t.smoothDetail : opts.smooth > 66 ? t.smoothSmooth : t.smoothMedium}</b></label>
						<input id="smooth" type="range" min={0} max={100} value={opts.smooth} onChange={e => set({ smooth: Number(e.target.value) }, t.smoothness)} />
						<div className="hint">{t.smoothHint}</div>
						<label htmlFor="target">{t.targetNodesLabel}</label>
						<select id="target" value={targetChoice} onChange={e => { recordHistory(t.targetNodesLabel); setTargetChoice(e.target.value as TargetChoice); }}>
							<option value="auto">{t.targetAuto}</option>
							<option value="500">500</option>
							<option value="1000">1000</option>
							<option value="2000">2000</option>
							<option value="custom">{t.targetCustom}</option>
						</select>
						{targetChoice === "custom" && <input className="num" type="number" min={1} step={50} value={customTarget} onChange={e => setCustomTarget(e.target.value)} aria-label={t.targetNodesLabel} />}
						<div className="hint">{t.targetHint}</div>
						<label className="check"><input type="checkbox" checked={opts.keepLines} onChange={e => set({ keepLines: e.target.checked }, t.keepLines)} /> {t.keepLines}</label>
						<label className="check"><input type="checkbox" checked={opts.detectPrimitives} onChange={e => set({ detectPrimitives: e.target.checked }, t.detectPrimitives)} /> {t.detectPrimitives}</label>
						<div className="hint">{t.primitivesHint}</div>
						<label htmlFor="symmetry">{t.symmetryLabel}</label>
						<select id="symmetry" value={opts.symmetry} onChange={e => set({ symmetry: e.target.value as Options["symmetry"] }, t.symmetryLabel)}>
							<option value="off">{t.symOff}</option>
							<option value="auto">{t.symAuto}</option>
							<option value="vertical">{t.symVert}</option>
							<option value="horizontal">{t.symHoriz}</option>
						</select>
						<div className="hint">{t.symmetryHint}</div>
						<label className="check"><input type="checkbox" checked={opts.aiClean} onChange={e => set({ aiClean: e.target.checked }, t.aiCleanLabel)} /> {t.aiCleanLabel}</label>
						<div className="hint">{t.aiCleanHint}</div>
						{opts.aiClean && !hasSidecar && <div className="warn">{t.noSidecarWarn}</div>}
						<button className={`primary ${dirty ? "dirty" : ""}`} onClick={apply} disabled={!path || busy}>{result ? t.applyAgain : t.vectorizeBtn}</button>
						{dirty && (
							<div className="pending" role="status">
								{t.pendingChanges}
								{palette !== null || strokes.length > 0 || hasPreprocess
									? ` (${[palette !== null ? t.pendingPalette : "", strokes.length > 0 ? `${strokes.length} ${t.pendingStrokesSuffix}` : "", hasPreprocess ? t.pendingPreprocess : ""].filter(Boolean).join(", ")})`
									: ""}.
							</div>
						)}

						{result && (
							<>
								<div className="palette-head">
									<h2>{t.paletteTitle}</h2>
									<div className="row">
										{hiddenLayers.size > 0 && (
											<button className="small" onClick={() => setHiddenLayers(new Set())}>{t.showAllLayers}</button>
										)}
										<button className="small" aria-pressed={tool === "pipet"} onClick={() => setTool(tl => (tl === "pipet" ? "pan" : "pipet"))}>{t.pipetBtn}</button>
										<button className="small" onClick={resetPalette} disabled={palette === null && !(result.palette.length > 0 && appliedSig.includes('"customPalette":["'))}>{t.resetPalette}</button>
									</div>
								</div>
								<ul className="palette" aria-label={t.paletteAria}>
									{currentPalette().map(c => {
										const isHidden = hiddenLayers.has(c);
										return (
											<li key={c} className={`swatch-item ${isHidden ? "layer-hidden" : ""}`}>
												<div className="swatch">
													<button
														className="small icon-btn"
														aria-pressed={isHidden}
														onClick={() => toggleLayer(c)}
														title={isHidden ? t.toggleLayerShow : t.toggleLayerHide}
														aria-label={`${isHidden ? t.toggleLayerShow : t.toggleLayerHide} ${c}`}
													>
														<EyeIcon hidden={isHidden} />
													</button>
													<span className="chip" style={{ background: c }} aria-hidden="true" />
													<code>{c}</code>
													<button className="small" aria-expanded={mergeSource === c} onClick={() => setMergeSource(s => (s === c ? null : c))} disabled={currentPalette().length <= 2} aria-label={`${t.mergeBtn} ${c}`}>{t.mergeBtn}</button>
													<button className="small" onClick={() => removeColor(c)} disabled={currentPalette().length <= 2} aria-label={`${t.removeBtn} ${c}`}>{t.removeBtn}</button>
												</div>
												{mergeSource === c && (
													<div className="merge-pop" role="region" aria-label={`${t.mergeToLabel} ${c}`}>
														<div className="hint">{t.mergeToLabel} <code>{c}</code></div>
														<div className="merge-targets">
															{othersByDistance(c).map((targetHex, i) => (
																<button key={targetHex} className="small merge-opt" onClick={() => mergeColorInto(c, targetHex)} aria-label={`${t.mergeBtn} ${c} → ${targetHex}`}>
																	<span className="chip-sm" style={{ background: targetHex }} aria-hidden="true" />
																	<span>{targetHex}{i === 0 ? t.nearestTag : ""}</span>
																</button>
															))}
														</div>
													</div>
												)}
											</li>
										);
									})}
									<li className={`swatch-item ${hiddenLayers.has(GARIS_LAYER) ? "layer-hidden" : ""}`}>
										<div className="swatch">
											<button
												className="small icon-btn"
												aria-pressed={hiddenLayers.has(GARIS_LAYER)}
												onClick={() => toggleLayer(GARIS_LAYER)}
												title={hiddenLayers.has(GARIS_LAYER) ? t.toggleLayerShow : t.toggleLayerHide}
												aria-label={t.layerThinLines}
											>
												<EyeIcon hidden={hiddenLayers.has(GARIS_LAYER)} />
											</button>
											<span className="chip chip-line" aria-hidden="true" />
											<span className="layer-label">{t.layerThinLines}</span>
										</div>
									</li>
									{hasRegLayer && (
										<li className={`swatch-item ${hiddenLayers.has(REG_LAYER) ? "layer-hidden" : ""}`}>
											<div className="swatch">
												<button
													className="small icon-btn"
													aria-pressed={hiddenLayers.has(REG_LAYER)}
													onClick={() => toggleLayer(REG_LAYER)}
													title={hiddenLayers.has(REG_LAYER) ? t.toggleLayerShow : t.toggleLayerHide}
													aria-label={t.layerRegistration}
												>
													<EyeIcon hidden={hiddenLayers.has(REG_LAYER)} />
												</button>
												<span className="chip chip-reg" aria-hidden="true" />
												<span className="layer-label">{t.layerRegistration}</span>
											</div>
										</li>
									)}
								</ul>
								<div className="hint">{t.layerPreviewOnlyHint}</div>
								<div className="hint">{t.paletteHintBase} {paletteDirty ? t.paletteDirtyHint : palette !== null ? t.paletteManualHint : t.paletteAutoHint}</div>

								<h2>{t.statsTitle}</h2>
								<dl className="stats">
									<dt>{t.statPaths}</dt><dd>{result.paths}</dd>
									<dt>{t.statNodes}</dt><dd>{result.nodes.toLocaleString(locale)}</dd>
									<dt>{t.statSvgSize}</dt><dd>{fmtKb(result.bytes)}</dd>
									<dt>{t.statPrimitives}</dt><dd>{result.primitives}</dd>
									{result.symmetryApplied && <><dt>{t.statSymmetry}</dt><dd>{result.symmetryApplied === "vertical" ? t.symVertShort : t.symHorizShort}</dd></>}
									{result.budgetMet !== null && <><dt>{t.statTargetNode}</dt><dd>{result.budgetMet ? t.budgetMet : t.budgetMissed}</dd><dt>{t.statEffSmooth}</dt><dd>{Math.round(result.effectiveSmooth)}</dd></>}
									<dt>{t.statMae}</dt><dd>{result.mae === null ? "-" : result.mae.toFixed(2)}</dd>
									<dt>{t.statTime}</dt><dd>{result.secs.toFixed(1)} {t.secUnit}{result.reused ? t.reusedSuffix : ""}</dd>
									<dt>{t.statCleaner}</dt><dd>{result.usedAi ? "Real-ESRGAN" : "median"}</dd>
								</dl>

								<h2>{t.exportTitle}</h2>
								<label>{t.colorSpaceLabel}</label>
								<div className="seg seg2" role="group" aria-label={t.colorSpaceLabel} style={{ marginTop: 4 }}>
									<button aria-pressed={exp.colorSpace === "rgb"} onClick={() => setExp(ex => ({ ...ex, colorSpace: "rgb" as ColorSpace }))}>RGB</button>
									<button aria-pressed={exp.colorSpace === "cmyk"} onClick={() => setExp(ex => ({ ...ex, colorSpace: "cmyk" as ColorSpace }))}>CMYK</button>
								</div>
								{exp.colorSpace === "cmyk" && <div className="hint">{t.colorSpaceHintCmyk}</div>}

								<label htmlFor="linesAs">{t.linesAsLabel}</label>
								<select id="linesAs" value={opts.productionMode === "cutEmbroidery" ? "shape" : exp.linesAs} disabled={opts.productionMode === "cutEmbroidery"} onChange={e => setExp({ ...exp, linesAs: e.target.value as ExportOptions["linesAs"] })}>
									<option value="stroke">{t.linesStroke}</option>
									<option value="shape">{t.linesShape}</option>
								</select>
								<label htmlFor="trap">{t.trapLabel}</label>
								<select id="trap" value={opts.productionMode === "cutEmbroidery" ? 0 : exp.trapPt} disabled={opts.productionMode === "cutEmbroidery"} onChange={e => setExp({ ...exp, trapPt: Number(e.target.value) })}>
									<option value={0}>{t.trapNone}</option>
									<option value={0.25}>0,25 pt</option>
									<option value={0.5}>0,5 pt</option>
									<option value={1}>1 pt</option>
									<option value={1.5}>1,5 pt</option>
									<option value={2}>2 pt</option>
								</select>
								{opts.productionMode === "cutEmbroidery" && <div className="hint">{t.cutExportHint}</div>}

								<label htmlFor="dxfUnit">{t.dxfUnitLabel}</label>
								<select id="dxfUnit" value={exp.dxfUnit} onChange={e => setExp({ ...exp, dxfUnit: e.target.value as DxfUnit })}>
									<option value="mm">{t.dxfUnitMm}</option>
									<option value="px">{t.dxfUnitPx}</option>
								</select>
								{exp.dxfUnit === "mm" && (
									<>
										<label htmlFor="dxfTargetWidth">{t.dxfTargetWidthLabel}</label>
										<input
											id="dxfTargetWidth"
											className="num"
											type="number"
											min={1}
											step={1}
											placeholder={t.dxfTargetWidthPlaceholder}
											value={dxfWidthInput}
											onChange={e => setDxfWidthInput(e.target.value)}
										/>
									</>
								)}

								<label className="check"><input type="checkbox" checked={withAse} onChange={e => setWithAse(e.target.checked)} /> {t.includeAse}</label>
								<div className="row" style={{ marginTop: 8 }}>
									<button onClick={() => void doCopySvg()} title={t.copySvgTip}>{t.copySvgBtn}</button>
									<button onClick={() => void exportAs("svg")}>{t.exportSvg}</button>
									<button onClick={() => void exportAs("pdf")}>{t.exportPdf}{exp.colorSpace === "cmyk" ? " (CMYK)" : ""}</button>
									<button onClick={() => void exportAs("dxf")}>{t.exportDxf}</button>
									<button onClick={() => void exportAs("eps")}>{t.exportEps}{exp.colorSpace === "cmyk" ? " (CMYK)" : ""}{!license.active ? " (Pro)" : ""}</button>
									<button onClick={() => void exportAs("ase")}>{t.exportAse}{exp.colorSpace === "cmyk" ? " (CMYK)" : ""}</button>
								</div>
								{opts.productionMode === "screenPrint" && (
									<div className="sep-box">
										<label className="check"><input type="checkbox" checked={sepPdf} onChange={e => setSepPdf(e.target.checked)} /> {t.includeSepPdf}</label>
										<button style={{ width: "100%", marginTop: 6 }} onClick={() => void exportSeparationFilms()}>{t.exportSepBtn}{!license.active ? " (Pro)" : ""}</button>
									</div>
								)}
							</>
						)}
						{result && result.textRegions.length > 0 && (
							<div className="warn" role="status" id="text-warning">
								{t.textWarnPrefix}{result.textRegions.length}{t.textWarnSuffix}
								<div className="row" style={{ marginTop: 8 }}>
									<button aria-pressed={showText} onClick={() => setShowText(s => !s)}>{showText ? t.hideTextAreas : t.highlightTextAreas}</button>
								</div>
							</div>
						)}
						{result?.notes.map(n => <div className="warn" key={n}>{n}</div>)}
						{error && <div className="error" role="alert">{error}</div>}
						{info && <div className="ok" role="status">{info}</div>}
					</aside>
				</main>
			)}

			{cropOpen && path && (
				<CropModal
					path={path}
					initial={opts.preprocess}
					onCancel={() => setCropOpen(false)}
					onApply={applyPreprocess}
				/>
			)}

			{modalOpen !== null && (
				<div className="modal" role="dialog" aria-modal="true" aria-label={t.privacyLicense} onClick={() => setModalOpen(null)}>
					<div className="modal-box" onClick={e => e.stopPropagation()}>
						<div className="row between">
							<h2>{t.privacyLicense}</h2>
							<button className="ghost" onClick={() => setModalOpen(null)}>{t.close}</button>
						</div>
						<Licenses
							markdown={modalOpen.markdown}
							license={license}
							initialTab={modalOpen.tab}
							promptHint={modalOpen.hint}
							onLicenseChanged={setLicense}
						/>
					</div>
				</div>
			)}
		</div>
	);
}

export default function App() {
	const [lang, setLangState] = useState<Lang>("id");

	useEffect(() => {
		void getAppSettings()
			.then(s => {
				if (s.language === "en" || s.language === "id") setLangState(s.language);
			})
			.catch(() => {});
	}, []);

	const setLang = useCallback((next: Lang) => {
		setLangState(next);
		void saveAppSettings({ language: next }).catch(() => {});
	}, []);

	return (
		<I18nProvider lang={lang} setLang={setLang}>
			<TraseaApp />
		</I18nProvider>
	);
}
