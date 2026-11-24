import {
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
} from "react";
import BatchView from "./BatchView";
import CropModal from "./CropModal";
import logoIcon from "./assets/trasea-icon-mini.svg";
import { NodeOverlay, type NodeSelection } from "./NodeOverlay";
import SettingsModal, { type SettingsTab } from "./SettingsModal";
import {
	IconChecker,
	IconCrop,
	IconDetailBrush,
	IconEyedropper,
	IconGear,
	IconHand,
	IconHeatmap,
	IconOutline,
	IconPointer,
	IconRedo,
	IconSmoothBrush,
	IconSoftProof,
	IconUndo,
	IconNode,
} from "./ToolbarIcons";
import {
	collectDroppedImages,
	copySvgToClipboard,
	checkForUpdate,
	DEFAULT_APP_SETTINGS,
	openExternalUrl,
	DEFAULT_PREPROCESS,
	deletePreset,
	deleteSavedPalette,
	exportDxf,
	getAppSettings,
	getSampleImagePath,
	listPresets,
	listSavedPalettes,
	loadProjectFile,
	onDrop,
	onProgress,
	openFolder,
	pasteClipboardImage,
	pickColor,
	pickFolder,
	pickImage,
	pickImageOrProject,
	pickPaletteTextSavePath,
	pickProjectSavePath,
	pickSavePath,
	processImage,
	renderPreviewSvg,
	saveAppSettings,
	saveFile,
	savePaletteLibraryItem,
	savePreset,
	saveProjectFile,
	saveSeparations,
	saveTextFile,
	sidecarAvailable,
	type AppSettings,
	type BrushStroke,
	type ColorSpace,
	type DxfUnit,
	type ExportKind,
	type ExportOptions,
	type UpdateInfo,
	type Options,
	type PaletteTextExportKind,
	type PathEdit,
	type NodeEdit,
	type PathNodeGeometry,
	inspectPathNodes,
	type PreprocessOptions,
	type Processed,
	type ProductionMode,
	type SavedPalette,
	type SpotColorMapping,
	type UserPreset,
} from "./api";
import {
	buildCssPalette,
	buildGplPalette,
	buildJsonPalette,
	effectiveShortcuts,
	formatColorCodes,
	matchShortcut,
} from "./colorUtils";
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
	centerlineInvert: false,
	centerlineStrokePt: 1.0,
	pathEdits: [],
	nodeEdits: [],
	editFingerprint: null,
};

const DEFAULT_EXP: ExportOptions = {
	linesAs: "stroke",
	trapPt: 0,
	registrationMarks: false,
	stripBackground: false,
	colorSpace: "rgb",
	dxfUnit: "mm",
	dxfTargetWidthMm: null,
	dieCutContour: false,
	dieCutOffsetMm: 2.0,
	dieCutSpotName: "CutContour",
	dieCutBridgeIslands: true,
	spotColors: [],
	stickerBacking: false,
	stickerBackingColor: "#FFFFFF",
	stickerBleedMm: 0.0,
};

type View = "editor" | "batch";
type Tool = "pan" | "selectPath" | "node" | "detail" | "smooth" | "pipet";
type PanelTab = "trace" | "colors" | "export";

function TipDot({ text }: { text: string }) {
	return (
		<span className="tip-dot" title={text} aria-label={text}>
			?
		</span>
	);
}

const GARIS_LAYER = "__GARIS__";
const REG_LAYER = "__REGISTRASI__";
const CUT_LAYER = "__CUTCONTOUR__";

const baseName = (p: string): string => (p.split(/[\\/]/).pop() ?? "gambar").replace(/\.[^.]+$/, "");
const parentDir = (p: string): string => p.replace(/[\\/][^\\/]+$/, "");
const fmtKb = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
const rgb = (hex: string): [number, number, number] => [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
const heatColor = (r: number): string => `hsl(${Math.round(120 * (1 - Math.min(1, Math.max(0, r))))}, 90%, 50%)`;

const normalizeOptions = (po: Partial<Options>): Options => ({
	...DEFAULTS,
	...po,
	sablonKnockout: po.sablonKnockout ?? false,
	centerlineInvert: po.centerlineInvert ?? false,
	centerlineStrokePt: po.centerlineStrokePt ?? 1.0,
	pathEdits: Array.isArray(po.pathEdits) ? po.pathEdits : [],
	nodeEdits: Array.isArray(po.nodeEdits) ? po.nodeEdits : [],
	editFingerprint: typeof po.editFingerprint === "string" ? po.editFingerprint : null,
	preprocess: po.preprocess
		? {
				crop: po.preprocess.crop ?? null,
				rotateDeg: po.preprocess.rotateDeg ?? 0,
				removeBackground: po.preprocess.removeBackground ?? false,
				bgTolerance: po.preprocess.bgTolerance ?? 24,
				defringePx: po.preprocess.defringePx ?? 1,
			}
		: { ...DEFAULT_PREPROCESS },
});

const normalizeExport = (pe: Partial<ExportOptions>): ExportOptions => ({
	...DEFAULT_EXP,
	...pe,
	colorSpace: pe.colorSpace ?? "rgb",
	dxfUnit: pe.dxfUnit ?? "mm",
	dxfTargetWidthMm: pe.dxfTargetWidthMm ?? null,
	dieCutContour: pe.dieCutContour ?? false,
	dieCutOffsetMm: pe.dieCutOffsetMm ?? 2.0,
	dieCutSpotName: pe.dieCutSpotName ?? "CutContour",
	dieCutBridgeIslands: pe.dieCutBridgeIslands ?? true,
	spotColors: Array.isArray(pe.spotColors) ? pe.spotColors : [],
	stickerBacking: pe.stickerBacking ?? false,
	stickerBackingColor: pe.stickerBackingColor ?? "#FFFFFF",
	stickerBleedMm: pe.stickerBleedMm ?? 0.0,
});

function applySoftProofToSvg(svg: string, proofMap: Map<string, string>): string {
	if (proofMap.size === 0) return svg;
	// Preserve #CutContour magenta intact while replacing all palette fills/strokes
	const cutMarker = '<g id="CutContour"';
	const cutIdx = svg.indexOf(cutMarker);
	const body = cutIdx >= 0 ? svg.slice(0, cutIdx) : svg;
	const tail = cutIdx >= 0 ? svg.slice(cutIdx) : "";
	const swapped = body.replace(/(fill|stroke)="(#[0-9A-Fa-f]{6})"/g, (m, attr: string, hex: string) => {
		const mapped = proofMap.get(hex.toUpperCase());
		return mapped ? `${attr}="${mapped}"` : m;
	});
	return swapped + tail;
}

function buildPreviewSvg(
	rawSvg: string,
	outline: boolean,
	hidden: Set<string>,
	softProof: boolean,
	proofMap: Map<string, string>,
	selectedPathId: string | null,
): string {
	let svg = softProof ? applySoftProofToSvg(rawSvg, proofMap) : rawSvg;
	const rules: string[] = [];
	if (outline) {
		rules.push(
			"path, circle, ellipse, rect, polygon, polyline { fill: none !important; stroke: #2563eb !important; stroke-width: 1.25px !important; vector-effect: non-scaling-stroke !important; }",
			"#Garis path { stroke: #0ea5e9 !important; }",
			"#CutContour path { stroke: #EC008C !important; stroke-width: 1.5px !important; }",
			"#Registrasi circle, #Registrasi line { stroke: #ef4444 !important; }",
		);
	}
	if (selectedPathId) {
		const safe = selectedPathId.replace(/["\\]/g, "");
		rules.push(
			`[data-trasea-id="${safe}"] { stroke: #2563eb !important; stroke-width: 2.5px !important; stroke-dasharray: 5 3 !important; vector-effect: non-scaling-stroke !important; }`,
		);
	}
	for (const key of hidden) {
		if (key === GARIS_LAYER) {
			rules.push("#Garis { display: none !important; }");
		} else if (key === REG_LAYER) {
			rules.push("#Registrasi { display: none !important; }");
		} else if (key === CUT_LAYER) {
			rules.push("#CutContour { display: none !important; }");
		} else if (/^#[0-9A-Fa-f]{6}$/.test(key)) {
			const suffix = key.slice(1).toUpperCase();
			rules.push(`g[id$="_${suffix}"] { display: none !important; }`);
		}
	}
	if (rules.length === 0) return svg;
	const styleTag = `<style>${rules.join(" ")}</style>`;
	const idx = svg.indexOf(">", svg.indexOf("<svg"));
	if (idx === -1) return svg;
	return svg.slice(0, idx + 1) + styleTag + svg.slice(idx + 1);
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

interface TraseaAppProps {
	appSettings: AppSettings;
	onUpdateSettings: (next: AppSettings) => void;
}

function TraseaApp({ appSettings, onUpdateSettings }: TraseaAppProps) {
	const { t, lang, setLang } = useI18n();
	const errText = useCallback((e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : t.unknownError), [t.unknownError]);

	const [view, setView] = useState<View>("editor");
	const [panelTab, setPanelTab] = useState<PanelTab>("trace");
	const [batchInitialFiles, setBatchInitialFiles] = useState<string[]>([]);
	const [path, setPath] = useState<string | null>(null);
	const [projectPath, setProjectPath] = useState<string | null>(null);
	const [missingSourceProject, setMissingSourceProject] = useState<string | null>(null);
	const [opts, setOpts] = useState<Options>(DEFAULTS);
	const [result, setResult] = useState<Processed | null>(null);
	const [liveSvg, setLiveSvg] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [stage, setStage] = useState("");
	const [fraction, setFraction] = useState(0);
	const [error, setError] = useState("");
	const [info, setInfo] = useState("");
	const [over, setOver] = useState(false);
	const [split, setSplit] = useState(50);
	const [exp, setExp] = useState<ExportOptions>(() => ({
		...DEFAULT_EXP,
		colorSpace: appSettings.defaultColorSpace,
		dxfUnit: appSettings.defaultDxfUnit,
		dieCutOffsetMm: appSettings.defaultDieCutOffsetMm,
		dieCutSpotName: appSettings.defaultDieCutSpotName,
		dieCutBridgeIslands: appSettings.defaultDieCutBridge,
	}));
	const [dxfWidthInput, setDxfWidthInput] = useState("");
	const [withAse, setWithAse] = useState(true);
	const [sepPdf, setSepPdf] = useState(false);
	const [checker, setChecker] = useState(false);
	const [outlineView, setOutlineView] = useState(false);
	const [softProof, setSoftProof] = useState(false);
	const [hiddenLayers, setHiddenLayers] = useState<Set<string>>(() => new Set());
	const [cropOpen, setCropOpen] = useState(false);
	const [settingsModal, setSettingsModal] = useState<{
		tab: SettingsTab;
		licenseSubTab?: "privacy" | "thirdParty";
	} | null>(null);
	const [updateBanner, setUpdateBanner] = useState<UpdateInfo | null>(null);
	useEffect(() => {
		if (!appSettings.autoCheckUpdate) return;
		let alive = true;
		void checkForUpdate()
			.then(info => {
				if (alive && info.hasUpdate) setUpdateBanner(info);
			})
			.catch(() => {});
		return () => { alive = false; };
	}, [appSettings.autoCheckUpdate]);
	const [showText, setShowText] = useState(false);
	const [showHeat, setShowHeat] = useState(false);
	const [tool, setTool] = useState<Tool>("pan");
	const [selectedPathId, setSelectedPathId] = useState<string | null>(null);
	const [brushSize, setBrushSize] = useState(40);
	const [strokes, setStrokes] = useState<BrushStroke[]>([]);
	const [live, setLive] = useState<BrushStroke | null>(null);
	const [pathEdits, setPathEdits] = useState<PathEdit[]>([]);
	const [nodeEdits, setNodeEdits] = useState<NodeEdit[]>([]);
	const [nodeGeo, setNodeGeo] = useState<PathNodeGeometry | null>(null);
	const [nodeSel, setNodeSel] = useState<NodeSelection | null>(null);
	const [showAllHandles, setShowAllHandles] = useState(false);
	const keepSelectionRef = useRef(false);
	// Edits are tied to the trace they were made on (fingerprint of the unedited trace, from the backend).
	const editFpRef = useRef<string | null>(null);
	// The options of the last run whose edits were valid (so Undo can bring the old trace and its edits back).
	const lastEditRunRef = useRef<Options | null>(null);
	const [palette, setPalette] = useState<string[] | null>(null);
	const [paletteMerges, setPaletteMerges] = useState<Array<[string, string]>>([]);
	const [mergeSource, setMergeSource] = useState<string | null>(null);
	const [inspectedColor, setInspectedColor] = useState<string | null>(null);
	const [copiedKey, setCopiedKey] = useState<string>("");
	const [savedPalettes, setSavedPalettes] = useState<SavedPalette[]>([]);
	const [selectedSavedPaletteId, setSelectedSavedPaletteId] = useState("");
	const [addingSavedPalette, setAddingSavedPalette] = useState(false);
	const [savedPaletteName, setSavedPaletteName] = useState("");
	const [targetChoice, setTargetChoice] = useState<TargetChoice>("auto");
	const [customTarget, setCustomTarget] = useState("800");
	const [hasSidecar, setHasSidecar] = useState(true);
	const [presets, setPresets] = useState<UserPreset[]>([]);
	const [selectedPresetId, setSelectedPresetId] = useState("");
	const [presetNameInput, setPresetNameInput] = useState("");
	const [addingPreset, setAddingPreset] = useState(false);
	const [history, setHistory] = useState<HistoryState>(emptyHistory);
	const [appliedSig, setAppliedSig] = useState("");

	const busyRef = useRef(false);
	const optsRef = useRef(opts);
	optsRef.current = opts;
	const settingsInitApplied = useRef(false);
	const standardBackupRef = useRef({ colors: DEFAULTS.colors, smooth: DEFAULTS.smooth });
	const overlayRef = useRef<SVGSVGElement | null>(null);
	const liveRef = useRef<BrushStroke | null>(null);

	const activeShortcuts = useMemo(() => effectiveShortcuts(appSettings.shortcuts), [appSettings.shortcuts]);

	// Sync initial loaded export defaults once after settings arrive from disk
	useEffect(() => {
		if (!settingsInitApplied.current && !path) {
			settingsInitApplied.current = true;
			setExp(ex => ({
				...ex,
				colorSpace: appSettings.defaultColorSpace,
				dxfUnit: appSettings.defaultDxfUnit,
				dieCutOffsetMm: appSettings.defaultDieCutOffsetMm,
				dieCutSpotName: appSettings.defaultDieCutSpotName,
				dieCutBridgeIslands: appSettings.defaultDieCutBridge,
			}));
		}
	}, [appSettings, path]);

	// Respect preferMedianOnly setting
	useEffect(() => {
		if (appSettings.preferMedianOnly && opts.aiClean) {
			setOpts(o => ({ ...o, aiClean: false }));
		}
	}, [appSettings.preferMedianOnly, opts.aiClean]);

	const targetNodes = useMemo((): number | null => {
		if (targetChoice === "auto") return null;
		const n = targetChoice === "custom" ? Number.parseInt(customTarget, 10) : Number.parseInt(targetChoice, 10);
		return Number.isFinite(n) && n > 0 ? n : null;
	}, [targetChoice, customTarget]);

	const makeSnapshot = useCallback(
		(label: string): EditorSnapshot => ({
			opts: {
				...opts,
				preprocess: {
					crop: opts.preprocess.crop ? [...opts.preprocess.crop] : null,
					rotateDeg: opts.preprocess.rotateDeg,
					removeBackground: opts.preprocess.removeBackground,
					bgTolerance: opts.preprocess.bgTolerance,
					defringePx: opts.preprocess.defringePx,
				},
			},
			palette: palette ? [...palette] : null,
			paletteMerges: paletteMerges.map(m => [m[0], m[1]]),
			strokes: strokes.map(s => ({ ...s, points: s.points.map(p => [p[0], p[1]]) })),
			pathEdits: pathEdits.map(pe => ({ ...pe })),
			nodeEdits: nodeEdits.map(ne => ({ ...ne })),
			editFingerprint: editFpRef.current,
			targetChoice,
			customTarget,
			exp: { ...exp, spotColors: exp.spotColors.map(s => ({ ...s })) },
			label,
		}),
		[opts, palette, paletteMerges, strokes, pathEdits, nodeEdits, targetChoice, customTarget, exp],
				);

	const recordHistory = useCallback(
		(label: string) => {
			setHistory(h => pushSnapshot(h, makeSnapshot(label), appSettings.undoLimit));
		},
		[makeSnapshot, appSettings.undoLimit],
	);

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
			const hasEdits = o.pathEdits.length + o.nodeEdits.length > 0;
			const effectiveOpts: Options = {
				...(appSettings.preferMedianOnly ? { ...o, aiClean: false } : o),
				editFingerprint: hasEdits ? editFpRef.current : null,
			};
			try {
				const r = await processImage(p, effectiveOpts);
				if (hasEdits && !r.editsStale) {
					// legacy projects have no fingerprint: adopt the one of the trace they were just applied to
					if (editFpRef.current === null) editFpRef.current = r.traceFingerprint;
					lastEditRunRef.current = { ...effectiveOpts, editFingerprint: editFpRef.current };
				}
				setResult(r);
				setLiveSvg(null);
				setHiddenLayers(new Set());
				if (!keepSelectionRef.current) setSelectedPathId(null);
				keepSelectionRef.current = false;
				setAppliedSig(JSON.stringify(effectiveOpts));
			} catch (e) {
				setError(errText(e));
			} finally {
				busyRef.current = false;
				setBusy(false);
			}
		},
		[errText, t.loadingImage, appSettings.preferMedianOnly],
	);

	const restoreSnapshot = useCallback(
		(snap: EditorSnapshot, prevPathEdits: PathEdit[], prevNodeEdits: NodeEdit[]) => {
			const normOpts = normalizeOptions(snap.opts);
			const normExp = normalizeExport(snap.exp);
			const restoredEdits = snap.pathEdits ?? [];
			const restoredNodeEdits = snap.nodeEdits ?? [];
			setOpts(normOpts);
			setPalette(snap.palette);
			setPaletteMerges(snap.paletteMerges);
			setStrokes(snap.strokes);
			setPathEdits(restoredEdits);
			setNodeEdits(restoredNodeEdits);
			editFpRef.current = snap.editFingerprint ?? null;
			setNodeSel(null);
			setTargetChoice(snap.targetChoice);
			setCustomTarget(snap.customTarget);
			setExp(normExp);
			setDxfWidthInput(normExp.dxfTargetWidthMm ? String(normExp.dxfTargetWidthMm) : "");
			setChecker((normOpts.productionMode === "cutEmbroidery" && normOpts.stripBackground) || normOpts.preprocess.removeBackground);
			setMergeSource(null);
			setSelectedPathId(null);
			// Instant re-render when undoing/redoing path edits (uses fast prep cache!)
			if (path && (JSON.stringify(prevPathEdits) !== JSON.stringify(restoredEdits) || JSON.stringify(prevNodeEdits) !== JSON.stringify(restoredNodeEdits))) {
				keepSelectionRef.current = false;
				const tNodes =
					snap.targetChoice === "auto"
						? null
						: Number.parseInt(snap.targetChoice === "custom" ? snap.customTarget : snap.targetChoice, 10) || null;
				void run(path, {
					...normOpts,
					customPalette: snap.palette,
					paletteMerges: snap.paletteMerges,
					targetNodes: tNodes,
					brushStrokes: snap.strokes,
					pathEdits: restoredEdits,
					nodeEdits: restoredNodeEdits,
				});
			}
		},
		[path, run],
	);

	const doUndo = useCallback(() => {
		const res = stepUndo(history, makeSnapshot(t.undo), appSettings.undoLimit);
		if (!res) return;
		const prevEdits = pathEdits;
		setHistory(res.history);
		restoreSnapshot(res.target, prevEdits, nodeEdits);
		setInfo(`Undo: ${res.target.label}`);
	}, [history, makeSnapshot, appSettings.undoLimit, pathEdits, nodeEdits, restoreSnapshot, t.undo]);

	const doRedo = useCallback(() => {
		const res = stepRedo(history, makeSnapshot(t.redo), appSettings.undoLimit);
		if (!res) return;
		const prevEdits = pathEdits;
		setHistory(res.history);
		restoreSnapshot(res.target, prevEdits, nodeEdits);
		setInfo(`Redo: ${res.target.label}`);
	}, [history, makeSnapshot, appSettings.undoLimit, pathEdits, nodeEdits, restoreSnapshot, t.redo]);

	const signature = useMemo(
		() => JSON.stringify({ ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes, pathEdits, nodeEdits }),
		[opts, palette, paletteMerges, targetNodes, strokes, pathEdits, nodeEdits],
	);

	const openPrivacyModal = useCallback((licenseSubTab: "privacy" | "thirdParty" = "privacy") => {
		setSettingsModal({ tab: "privacy", licenseSubTab });
	}, []);

	const effectiveExport = useCallback((): ExportOptions => {
		const parsedW = Number.parseFloat(dxfWidthInput);
		let dxfTargetWidthMm: number | null = exp.dxfUnit === "mm" && Number.isFinite(parsedW) && parsedW > 0 ? parsedW : null;
		if (dxfTargetWidthMm === null && exp.dxfUnit === "mm" && appSettings.defaultDpi !== 96 && result) {
			dxfTargetWidthMm = Number(((result.width * 25.4) / appSettings.defaultDpi).toFixed(3));
		}
		const base: ExportOptions = { ...exp, dxfTargetWidthMm };
		if (opts.productionMode === "cutEmbroidery") {
			return { ...base, linesAs: "shape", trapPt: 0, registrationMarks: false, stripBackground: opts.stripBackground };
		}
		if (opts.productionMode === "centerline") {
			return { ...base, linesAs: "stroke", trapPt: 0, registrationMarks: false, stripBackground: true };
		}
		if (opts.productionMode === "screenPrint") {
			return { ...base, registrationMarks: opts.registrationMarks, stripBackground: false };
		}
		return { ...base, registrationMarks: false, stripBackground: opts.preprocess.removeBackground };
	}, [exp, dxfWidthInput, appSettings.defaultDpi, result, opts.productionMode, opts.stripBackground, opts.registrationMarks, opts.preprocess.removeBackground]);

	// Live preview refresh for Die-Cut Contour & export preview changes (no re-vectorization!)
	useEffect(() => {
		if (!result) return;
		const eff = effectiveExport();
		const needsLive =
			eff.dieCutContour ||
			eff.registrationMarks !== opts.registrationMarks ||
			eff.stripBackground !== (opts.productionMode === "cutEmbroidery" ? opts.stripBackground : false) ||
			eff.trapPt > 0 ||
			eff.linesAs === "shape";
		if (!needsLive) {
			setLiveSvg(null);
			return;
		}
		let alive = true;
		const timer = window.setTimeout(() => {
			void renderPreviewSvg(eff)
				.then(svg => {
					if (alive) setLiveSvg(svg);
				})
				.catch(() => {});
		}, 120);
		return () => {
			alive = false;
			window.clearTimeout(timer);
		};
	}, [
		result,
		exp.dieCutContour,
		exp.dieCutOffsetMm,
		exp.dieCutSpotName,
		exp.dieCutBridgeIslands,
		exp.stickerBacking,
		exp.stickerBackingColor,
		exp.stickerBleedMm,
		exp.linesAs,
		exp.trapPt,
		opts.registrationMarks,
		opts.stripBackground,
		opts.productionMode,
		opts.preprocess.removeBackground,
		effectiveExport,
	]);

	const openPicture = useCallback(
		(p: string) => {
			setView("editor");
			setProjectPath(null);
			setMissingSourceProject(null);
			setStrokes([]);
			setLive(null);
			setPathEdits([]);
			setNodeEdits([]);
			editFpRef.current = null;
			lastEditRunRef.current = null;
			setNodeSel(null);
			setNodeGeo(null);
			setSelectedPathId(null);
			setPalette(null);
			setPaletteMerges([]);
			setMergeSource(null);
			setInspectedColor(null);
			setHiddenLayers(new Set());
			setHistory(emptyHistory());
			setTool("pan");
			const nextOpts: Options = {
				...optsRef.current,
				customPalette: null,
				paletteMerges: [],
				brushStrokes: [],
				pathEdits: [],
				nodeEdits: [],
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
				setPathEdits(po.pathEdits ?? []);
				setNodeEdits(po.nodeEdits ?? []);
				editFpRef.current = po.editFingerprint ?? null;
				lastEditRunRef.current = null;
setNodeSel(null);
setChecker((po.productionMode === "cutEmbroidery" && po.stripBackground) || po.preprocess.removeBackground);
				setHiddenLayers(new Set());
				setHistory(emptyHistory());
				setSelectedPathId(null);
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
		const mergedOpts: Options = { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes, pathEdits, nodeEdits };
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
				const fullOpts: Options = { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes, pathEdits, nodeEdits, editFingerprint: pathEdits.length + nodeEdits.length > 0 ? editFpRef.current : null };
				await saveProjectFile(dest, path, fullOpts, effectiveExport());
				setProjectPath(dest);
				setInfo(`${t.projectSaved} ${dest}`);
			} catch (e) {
				setError(errText(e));
			}
		},
		[path, projectPath, opts, palette, paletteMerges, targetNodes, strokes, pathEdits, nodeEdits, effectiveExport, errText, t.projectSaved],
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
		void listPresets().then(setPresets).catch(() => {});
		void listSavedPalettes().then(setSavedPalettes).catch(() => {});
		const subs: Array<Promise<() => void>> = [
			onProgress((s, f) => {
				setStage(s);
				setFraction(f);
			}),
			onDrop(paths => {
				const proj = paths.find(x => x.toLowerCase().endsWith(".trasea"));
				if (paths.length === 1 && paths[0]?.toLowerCase().endsWith(".svg")) {
					openPicture(paths[0]);
					return;
				}
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

	// Path Edit helpers (F4)
	const applyPathEditImmediate = useCallback(
		(edit: PathEdit, label: string) => {
			if (!path || busyRef.current) return;
			recordHistory(label);
			if (editFpRef.current === null && result) editFpRef.current = result.traceFingerprint;
			const nextEdits = [...pathEdits, edit];
			setPathEdits(nextEdits);
			setSelectedPathId(null);
			void run(path, {
				...opts,
				customPalette: palette,
				paletteMerges,
				targetNodes,
				brushStrokes: strokes,
				pathEdits: nextEdits,
				nodeEdits,
			});
		},
		[path, recordHistory, pathEdits, nodeEdits, result, run, opts, palette, paletteMerges, targetNodes, strokes],
	);

	const deleteSelectedPath = useCallback(() => {
		if (!selectedPathId) return;
		applyPathEditImmediate({ kind: "delete", targetId: selectedPathId }, `${t.pathDeleteBtn} ${selectedPathId}`);
	}, [selectedPathId, applyPathEditImmediate, t.pathDeleteBtn]);

	const recolorSelectedPath = useCallback(
		(toHex: string) => {
			if (!selectedPathId) return;
			applyPathEditImmediate(
				{ kind: "reassignColor", targetId: selectedPathId, toHex },
				`${t.pathRecolorLabel} ${selectedPathId} → ${toHex}`,
			);
		},
		[selectedPathId, applyPathEditImmediate, t.pathRecolorLabel],
	);

	const resetAllPathEdits = useCallback(() => {
		if (!path || pathEdits.length === 0 || busyRef.current) return;
		recordHistory(t.pathResetEdits);
		if (nodeEdits.length === 0) editFpRef.current = null;
		setPathEdits([]);
		setSelectedPathId(null);
		void run(path, {
			...opts,
			customPalette: palette,
			paletteMerges,
			targetNodes,
			brushStrokes: strokes,
							pathEdits: [],
							nodeEdits,
						});
					}, [path, pathEdits.length, nodeEdits, recordHistory, t.pathResetEdits, run, opts, palette, paletteMerges, targetNodes, strokes]);

	// An edit belongs to the trace it was made on. When the backend reports the sent edits stale (the trace changed), they were not
	// applied; drop them here, keep a history step that brings the old trace and its edits back, and tell the user.
	useEffect(() => {
		if (!result?.editsStale) return;
		const prev = lastEditRunRef.current;
		if (prev) {
			const tn = prev.targetNodes;
			const snap: EditorSnapshot = {
				opts: prev,
				palette: prev.customPalette,
				paletteMerges: prev.paletteMerges,
				strokes: prev.brushStrokes,
				pathEdits: prev.pathEdits,
				nodeEdits: prev.nodeEdits,
				editFingerprint: prev.editFingerprint,
				targetChoice: tn === null ? "auto" : [500, 1000, 2000].includes(tn) ? (String(tn) as TargetChoice) : "custom",
				customTarget: tn === null ? "" : String(tn),
				exp,
				label: t.editsResetLabel,
			};
			setHistory(h => pushSnapshot(h, snap, appSettings.undoLimit));
		}
		lastEditRunRef.current = null;
		editFpRef.current = null;
		setPathEdits([]);
		setNodeEdits([]);
		setNodeSel(null);
		setSelectedPathId(null);
		setInfo(t.editsResetNotice);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [result]);

	// ---- Node tool (1.3.0) ----
	// geometry of the selected path (source pixels); refreshed after every re-render
	useEffect(() => {
		if (tool !== "node" || !selectedPathId || !result) {
			setNodeGeo(null);
			return;
		}
		let cancelled = false;
		void inspectPathNodes(selectedPathId)
			.then(g => {
				if (!cancelled) setNodeGeo(g);
			})
			.catch(() => {
				if (!cancelled) setNodeGeo(null);
			});
		return () => {
			cancelled = true;
		};
	}, [tool, selectedPathId, result]);

	const commitNodeEdit = useCallback(
		(edit: NodeEdit, label: string) => {
			if (!path || busyRef.current) return;
			recordHistory(label);
			if (editFpRef.current === null && result) editFpRef.current = result.traceFingerprint;
			const next = [...nodeEdits, edit];
			setNodeEdits(next);
			keepSelectionRef.current = true;
			void run(path, {
				...opts,
				customPalette: palette,
				paletteMerges,
				targetNodes,
				brushStrokes: strokes,
				pathEdits,
				nodeEdits: next,
			});
		},
		[path, recordHistory, nodeEdits, result, run, opts, palette, paletteMerges, targetNodes, strokes, pathEdits],
	);

	const onNodeMove = useCallback(
		(sub: number, idx: number, x: number, y: number) => {
			if (!selectedPathId) return;
			commitNodeEdit({ kind: "moveNode", targetId: selectedPathId, subpathIdx: sub, nodeIdx: idx, x, y }, t.nodeMoveLabel);
		},
		[selectedPathId, commitNodeEdit, t.nodeMoveLabel],
	);
	const onNodeMoveHandle = useCallback(
		(sub: number, idx: number, which: "in" | "out", x: number, y: number, mirrorSmooth: boolean) => {
			if (!selectedPathId) return;
			commitNodeEdit(
				{ kind: "moveHandle", targetId: selectedPathId, subpathIdx: sub, nodeIdx: idx, which, x, y, mirrorSmooth },
				t.nodeMoveHandleLabel,
			);
		},
		[selectedPathId, commitNodeEdit, t.nodeMoveHandleLabel],
	);
	const onNodeToggle = useCallback(
		(sub: number, idx: number) => {
			if (!selectedPathId) return;
			commitNodeEdit({ kind: "toggleSmooth", targetId: selectedPathId, subpathIdx: sub, nodeIdx: idx }, t.nodeToggleLabel);
		},
		[selectedPathId, commitNodeEdit, t.nodeToggleLabel],
	);
	const deleteSelectedNode = useCallback(() => {
		if (!selectedPathId || !nodeSel) return;
		commitNodeEdit(
			{ kind: "deleteNode", targetId: selectedPathId, subpathIdx: nodeSel.sub, nodeIdx: nodeSel.idx },
			t.nodeDeleteLabel,
		);
		setNodeSel(null);
	}, [selectedPathId, nodeSel, commitNodeEdit, t.nodeDeleteLabel]);
	const simplifySelectedPath = useCallback(() => {
		if (!selectedPathId) return;
		commitNodeEdit({ kind: "simplifyPath", targetId: selectedPathId, tolerancePx: 1.0 }, t.nodeSimplifyLabel);
		setNodeSel(null);
	}, [selectedPathId, commitNodeEdit, t.nodeSimplifyLabel]);
	const resetNodeEdits = useCallback(() => {
		if (!path || nodeEdits.length === 0 || busyRef.current) return;
		recordHistory(t.nodeResetLabel);
		if (pathEdits.length === 0) editFpRef.current = null;
		setNodeEdits([]);
		setNodeSel(null);
		keepSelectionRef.current = true;
		void run(path, {
			...opts,
			customPalette: palette,
			paletteMerges,
			targetNodes,
			brushStrokes: strokes,
			pathEdits,
			nodeEdits: [],
		});
	}, [path, nodeEdits.length, recordHistory, t.nodeResetLabel, run, opts, palette, paletteMerges, targetNodes, strokes, pathEdits]);

	// Global keyboard shortcuts driven by SettingsModal (`activeShortcuts`)
	useEffect(() => {
		const onKey = (e: KeyboardEvent) => {
			if (settingsModal !== null) return;
			const tag = (e.target as HTMLElement | null)?.tagName ?? "";
			const editingText = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";

			if (!editingText && (e.key === "Delete" || e.key === "Backspace") && view === "editor" && tool === "node" && selectedPathId && nodeSel) {
				e.preventDefault();
				deleteSelectedNode();
				return;
			}

			if (!editingText && (e.key === "Delete" || e.key === "Backspace") && view === "editor" && tool === "selectPath" && selectedPathId) {
				e.preventDefault();
				deleteSelectedPath();
				return;
			}

			if (matchShortcut(e, activeShortcuts.openSettings)) {
				e.preventDefault();
				setSettingsModal({ tab: "general" });
			} else if (matchShortcut(e, activeShortcuts.redo) && !editingText && view === "editor") {
				e.preventDefault();
				doRedo();
			} else if (matchShortcut(e, activeShortcuts.undo) && !editingText && view === "editor") {
				e.preventDefault();
				doUndo();
			} else if (matchShortcut(e, activeShortcuts.toggleOutline) && !editingText && view === "editor" && result) {
				e.preventDefault();
				setOutlineView(v => !v);
			} else if (matchShortcut(e, activeShortcuts.togglePathSelect) && !editingText && view === "editor" && result) {
				e.preventDefault();
				setTool(tl => (tl === "selectPath" ? "pan" : "selectPath"));
			} else if (matchShortcut(e, activeShortcuts.nodeTool) && !editingText && view === "editor" && result) {
				e.preventDefault();
				setTool(tl => (tl === "node" ? "pan" : "node"));
			} else if (matchShortcut(e, activeShortcuts.toggleSoftProof) && !editingText && view === "editor" && result) {
				e.preventDefault();
				setSoftProof(v => !v);
			} else if (matchShortcut(e, activeShortcuts.paste) && !editingText && view === "editor" && !cropOpen) {
				e.preventDefault();
				void doPasteClipboard();
			} else if (matchShortcut(e, activeShortcuts.copySvg) && !editingText && view === "editor" && result) {
				e.preventDefault();
				void doCopySvg();
			} else if (matchShortcut(e, activeShortcuts.saveProject) && view === "editor" && path) {
				e.preventDefault();
				void saveProject(false);
			} else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "s" && view === "editor" && path) {
				e.preventDefault();
				void saveProject(true);
			} else if (matchShortcut(e, activeShortcuts.openFile) && !busyRef.current) {
				e.preventDefault();
				void choose();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	}, [
		settingsModal,
		activeShortcuts,
		view,
		path,
		result,
		cropOpen,
		tool,
		selectedPathId,
		nodeSel,
		deleteSelectedNode,
		deleteSelectedPath,
		doUndo,
		doRedo,
		doPasteClipboard,
		doCopySvg,
		saveProject,
		choose,
	]);

	const switchProductionMode = (mode: ProductionMode) => {
		const mName =
			mode === "screenPrint"
				? t.modeScreenPrint
				: mode === "cutEmbroidery"
					? t.modeCutEmbroidery
					: mode === "shading"
						? t.modeShading
						: mode === "centerline"
							? t.modeCenterline
							: t.modeStandard;
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
			if (mode === "centerline") {
				return {
					...o,
					productionMode: "centerline",
					minStrokePt: 0,
					stripBackground: true,
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
			if (!opts.preprocess.removeBackground) setChecker(false);
		} else if (mode === "screenPrint") {
			setExp(ex => ({ ...ex, linesAs: "stroke", trapPt: 0.5, registrationMarks: true, stripBackground: false }));
			if (!opts.preprocess.removeBackground) setChecker(false);
		} else if (mode === "centerline") {
			setExp(ex => ({ ...ex, linesAs: "stroke", trapPt: 0, registrationMarks: false, stripBackground: true }));
			setChecker(true);
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
		setOpts({ ...po, customPalette: palette, paletteMerges, brushStrokes: strokes, pathEdits, nodeEdits, preprocess: opts.preprocess });
		setExp(pe);
		setDxfWidthInput(pe.dxfTargetWidthMm ? String(pe.dxfTargetWidthMm) : "");
		setChecker((po.productionMode === "cutEmbroidery" && po.stripBackground) || po.preprocess.removeBackground);
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
				options: {
					...opts,
					customPalette: null,
					paletteMerges: [],
					brushStrokes: [],
					pathEdits: [],
					nodeEdits: [],
					targetNodes,
					preprocess: { ...DEFAULT_PREPROCESS },
				},
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
		setInspectedColor(null);
		if (path && !busyRef.current)
			void run(path, { ...opts, customPalette: null, paletteMerges: [], targetNodes, brushStrokes: strokes, pathEdits, nodeEdits });
		};

		const apply = () => {
		setMergeSource(null);
		if (path) void run(path, { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes, pathEdits, nodeEdits });
				};

	const applyPreprocess = (pp: PreprocessOptions) => {
		recordHistory(t.cropRotate);
		setCropOpen(false);
		setChecker(pp.removeBackground || (opts.productionMode === "cutEmbroidery" && opts.stripBackground));
		const nextOpts: Options = {
			...opts,
			preprocess: pp,
			customPalette: palette,
			paletteMerges,
			targetNodes,
			brushStrokes: [],
			pathEdits: [],
			nodeEdits: [],
		};
		setStrokes([]);
		setPathEdits([]);
		setNodeEdits([]);
		editFpRef.current = null;
		setNodeSel(null);
		setOpts(nextOpts);
		if (path) void run(path, nextOpts);
	};

	const resetPreprocess = () => {
		applyPreprocess({ ...DEFAULT_PREPROCESS });
	};

	const exportAs = async (kind: ExportKind) => {
		if (!path) return;
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
			if (appSettings.openFolderAfterExport) {
				void openFolder(parentDir(target)).catch(() => {});
			}
		} catch (e) {
			setError(errText(e));
		}
	};

	const exportPaletteText = async (kind: PaletteTextExportKind) => {
		const colors = currentPalette();
		if (colors.length === 0) return;
		setError("");
		setInfo("");
		const stem = path ? baseName(path) : "trasea-palette";
		try {
			const target = await pickPaletteTextSavePath(kind, `${stem}.${kind}`);
			if (!target) return;
			const content =
				kind === "gpl"
					? buildGplPalette(stem, colors, exp.spotColors)
					: kind === "css"
						? buildCssPalette(stem, colors, exp.spotColors)
						: buildJsonPalette(stem, colors, exp.spotColors);
			await saveTextFile(target, content);
			setInfo(`${t.savedLabel} ${target}`);
			if (appSettings.openFolderAfterExport) {
				void openFolder(parentDir(target)).catch(() => {});
			}
		} catch (e) {
			setError(errText(e));
		}
	};

	const exportSeparationFilms = async () => {
		if (!path) return;
		setError("");
		setInfo("");
		try {
			const dir = await pickFolder(t.chooseSepFolderTitle);
			if (!dir) return;
			const effExp: ExportOptions = { ...effectiveExport(), registrationMarks: true, stripBackground: false };
			const written = await saveSeparations(dir, baseName(path), sepPdf ? "both" : "svg", effExp);
			setInfo(`${t.savedSepPrefix} ${written.length} ${t.savedSepMid} ${dir}`);
			if (appSettings.openFolderAfterExport) {
				void openFolder(dir).catch(() => {});
			}
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

	const proofMap = useMemo(() => {
		const m = new Map<string, string>();
		for (const p of result?.paletteProof ?? []) {
			m.set(p.hex.toUpperCase(), p.proofHex.toUpperCase());
		}
		return m;
	}, [result]);

	const outOfGamutSet = useMemo(() => {
		const s = new Set<string>();
		for (const p of result?.paletteProof ?? []) {
			if (p.outOfGamut) s.add(p.hex.toUpperCase());
		}
		return s;
	}, [result]);

	const spotMap = useMemo(() => {
		const m = new Map<string, string>();
		for (const sc of exp.spotColors) {
			if (sc.spotName.trim()) m.set(sc.hex.toUpperCase(), sc.spotName.trim());
		}
		return m;
	}, [exp.spotColors]);

	const setSpotName = (hexColor: string, spotName: string) => {
		const upper = hexColor.toUpperCase();
		setExp(ex => {
			const filtered = ex.spotColors.filter(s => s.hex.toUpperCase() !== upper);
			const clean = spotName.trim();
			const next: SpotColorMapping[] = clean ? [...filtered, { hex: upper, spotName: clean }] : filtered;
			return { ...ex, spotColors: next };
		});
	};

	const copyCodeValue = async (key: string, value: string) => {
		try {
			await navigator.clipboard.writeText(value);
			setCopiedKey(key);
			window.setTimeout(() => setCopiedKey(k => (k === key ? "" : k)), 1400);
		} catch {
			setError(t.unknownError);
		}
	};

	const saveCurrentToPaletteLibrary = async () => {
		const name = savedPaletteName.trim();
		const colors = currentPalette();
		if (!name || colors.length === 0) return;
		setError("");
		try {
			const id = `pal-${Date.now()}`;
			const next = await savePaletteLibraryItem({
				id,
				name,
				colors: colors.map(hex => ({ hex, spotName: spotMap.get(hex.toUpperCase()) ?? null })),
			});
			setSavedPalettes(next);
			setSelectedSavedPaletteId(id);
			setSavedPaletteName("");
			setAddingSavedPalette(false);
			setInfo(t.paletteSavedMsg);
		} catch (e) {
			setError(errText(e));
		}
	};

	const applySavedPalette = (sp: SavedPalette) => {
		recordHistory(`${t.paletteLibTitle}: ${sp.name}`);
		setSelectedSavedPaletteId(sp.id);
		setPalette(sp.colors.map(c => c.hex.toUpperCase()));
		setPaletteMerges([]);
		const nextSpots: SpotColorMapping[] = sp.colors
			.filter(c => c.spotName && c.spotName.trim().length > 0)
			.map(c => ({ hex: c.hex.toUpperCase(), spotName: (c.spotName ?? "").trim() }));
		if (nextSpots.length > 0) {
			setExp(ex => ({ ...ex, spotColors: nextSpots }));
		}
		setInfo(t.paletteAppliedMsg);
	};

	const removeSelectedSavedPalette = async () => {
		if (!selectedSavedPaletteId) return;
		try {
			const next = await deleteSavedPalette(selectedSavedPaletteId);
			setSavedPalettes(next);
			setSelectedSavedPaletteId("");
			setInfo(t.paletteDeletedMsg);
		} catch (e) {
			setError(errText(e));
		}
	};

	const baseSvg = liveSvg ?? result?.svg ?? "";
	const previewSvg = useMemo(
		() => (baseSvg ? buildPreviewSvg(baseSvg, outlineView, hiddenLayers, softProof, proofMap, tool === "selectPath" || tool === "node" ? selectedPathId : null) : ""),
		[baseSvg, outlineView, hiddenLayers, softProof, proofMap, tool, selectedPathId],
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
		if (inspectedColor === hex) setInspectedColor(null);
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

	const onInteractiveSvgClick = (e: ReactMouseEvent<HTMLDivElement>) => {
		if (tool !== "selectPath" && tool !== "node") return;
		const el = (e.target as Element | null)?.closest("[data-trasea-id]");
		const id = el?.getAttribute("data-trasea-id") ?? null;
		if (tool === "node" && id === null) return; // keep the current path when clicking empty canvas
		setSelectedPathId(id);
		setNodeSel(null);
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

	const hasPreprocess = opts.preprocess.crop !== null || Math.abs(opts.preprocess.rotateDeg) > 0.05 || opts.preprocess.removeBackground;
	const hasRegLayer = baseSvg.includes('id="Registrasi"');
	const hasCutLayer = baseSvg.includes('id="CutContour"');
	const dirty = result !== null && appliedSig !== "" && signature !== appliedSig;
	const paletteDirty = result !== null && palette !== null && JSON.stringify(palette) !== JSON.stringify(result.palette);
	const painting = tool === "detail" || tool === "smooth";
	const shown = live ? [...strokes, live] : strokes;
	const maxColors = opts.productionMode === "screenPrint" ? 8 : opts.productionMode === "shading" ? 32 : 16;
	const minColors = opts.productionMode === "shading" ? 12 : 2;
	const undoTip =
		history.past.length > 0
			? `${t.undoTipPrefix} (${activeShortcuts.undo}): ${history.past[history.past.length - 1]?.label ?? ""}`
			: `${t.undoTipPrefix} (${activeShortcuts.undo})`;
	const redoTip =
		history.future.length > 0
			? `${t.redoTipPrefix} (${activeShortcuts.redo}): ${history.future[0]?.label ?? ""}`
			: `${t.redoTipPrefix} (${activeShortcuts.redo})`;
	const locale = lang === "en" ? "en-US" : "id-ID";
	const canvasBgClass = checker ? `checker bg-${appSettings.canvasBg}` : "";

	return (
		<div className={`app ${over ? "dragging" : ""}`} data-signature={signature.length}>
			<header className="top">
				<div className="brand">
					<img className="logo" src={logoIcon} alt="" aria-hidden="true" width={24} height={24} /> Trasea <small>{t.headerOffline}</small>
					{updateBanner && (
						<span className="update-pill" role="status">
							{t.updateBannerText} v{updateBanner.latestVersion}
							<button className="small primary" onClick={() => void openExternalUrl(updateBanner.downloadUrl).catch(() => {})}>{t.updateBannerBtn}</button>
							<button className="small ghost" onClick={() => setUpdateBanner(null)} aria-label={t.close}>×</button>
						</span>
					)}
				</div>
				<div className="row">
					<div className="seg seg2 lang-switch" role="group" aria-label={t.switchLangTip} title={t.switchLangTip}>
						<button aria-pressed={lang === "id"} onClick={() => setLang("id")}>ID</button>
						<button aria-pressed={lang === "en"} onClick={() => setLang("en")}>EN</button>
					</div>
					<button className="ghost" onClick={() => void doPasteClipboard()} disabled={busy} title={`${t.pasteImageTip} (${activeShortcuts.paste})`}>{t.pasteImage}</button>
					<button aria-pressed={view === "batch"} onClick={() => setView(v => (v === "batch" ? "editor" : "batch"))} disabled={busy}>
						{t.batchFolder}
					</button>
					{view === "editor" && path && (
						<button className="ghost" onClick={() => void saveProject(false)} disabled={busy} title={`${t.saveProjectTip} (${activeShortcuts.saveProject})`}>{t.saveProject}</button>
					)}
					<button onClick={() => void choose()} disabled={busy} title={`${t.openImageProjectTip} (${activeShortcuts.openFile})`}>{t.openImageProject}</button>
					<button
						className="ghost settings-gear-btn"
						onClick={() => setSettingsModal({ tab: "general" })}
						title={`${t.settingsBtnTip} (${activeShortcuts.openSettings})`}
						aria-label={t.settingsBtn}
					>
						<IconGear />
					</button>
				</div>
			</header>

			{view === "batch" ? (
				<BatchView
					opts={opts}
					exp={effectiveExport()}
					initialFiles={batchInitialFiles}
					presets={presets}
					onApplyPreset={applyPreset}
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
									<div className="tb-group" role="group" aria-label={t.modeGroupAria}>
										<button
											className="tb-icon-btn"
											aria-pressed={tool === "pan"}
											onClick={() => setTool("pan")}
											title={t.toolPan}
											aria-label={t.toolPan}
										>
											<IconHand />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={tool === "selectPath"}
											onClick={() => setTool(tl => (tl === "selectPath" ? "pan" : "selectPath"))}
											title={`${t.toolSelectPath} (${activeShortcuts.togglePathSelect}) — ${t.toolSelectPathTip}`}
											aria-label={`${t.toolSelectPath} (${activeShortcuts.togglePathSelect})`}
										>
											<IconPointer />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={tool === "node"}
											data-testid="tool-node"
											onClick={() => setTool(tl => (tl === "node" ? "pan" : "node"))}
											title={`${t.toolNode} (${activeShortcuts.nodeTool}) — ${t.toolNodeTip}`}
											aria-label={`${t.toolNode} (${activeShortcuts.nodeTool})`}
										>
											<IconNode />
										</button>
										<button
											className="tb-icon-btn tool-detail"
											aria-pressed={tool === "detail"}
											onClick={() => setTool("detail")}
											title={t.toolDetail}
											aria-label={t.toolDetail}
										>
											<IconDetailBrush />
										</button>
										<button
											className="tb-icon-btn tool-smooth"
											aria-pressed={tool === "smooth"}
											onClick={() => setTool("smooth")}
											title={t.toolSmooth}
											aria-label={t.toolSmooth}
										>
											<IconSmoothBrush />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={tool === "pipet"}
											onClick={() => setTool("pipet")}
											title={t.toolPipet}
											aria-label={t.toolPipet}
										>
											<IconEyedropper />
										</button>
									</div>

									<span className="tb-sep" aria-hidden="true" />

									<div className="tb-group" role="group" aria-label={t.historyGroupAria}>
										<button
											className="tb-icon-btn"
											onClick={doUndo}
											disabled={history.past.length === 0}
											title={undoTip}
											aria-label={undoTip}
										>
											<IconUndo />
										</button>
										<button
											className="tb-icon-btn"
											onClick={doRedo}
											disabled={history.future.length === 0}
											title={redoTip}
											aria-label={redoTip}
										>
											<IconRedo />
										</button>
									</div>

									{painting && (
										<>
											<span className="tb-sep" aria-hidden="true" />
											<div className="group">
												<label className="brushsize" htmlFor="brush">{t.brushSize} <b>{brushSize}</b> px</label>
												<input id="brush" type="range" min={10} max={120} value={brushSize} onChange={e => setBrushSize(Number(e.target.value))} />
												<button className="small" onClick={() => { recordHistory(t.clearStrokes); setStrokes([]); }} disabled={strokes.length === 0}>{t.clearStrokes}</button>
											</div>
										</>
									)}

									<span className="tb-sep tb-views-sep" aria-hidden="true" />

									<div className="tb-group tb-views" role="group" aria-label={t.overlayGroupAria}>
										<button
											className="tb-icon-btn"
											aria-pressed={outlineView}
											onClick={() => setOutlineView(v => !v)}
											title={`${t.outlineView} (${activeShortcuts.toggleOutline}) — ${t.outlineTip}`}
											aria-label={`${t.outlineView} (${activeShortcuts.toggleOutline})`}
										>
											<IconOutline />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={softProof}
											onClick={() => setSoftProof(v => !v)}
											title={`${t.softProofBtn} (${activeShortcuts.toggleSoftProof}) — ${t.softProofTip}`}
											aria-label={`${t.softProofBtn} (${activeShortcuts.toggleSoftProof})`}
										>
											<IconSoftProof />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={hasPreprocess}
											onClick={() => setCropOpen(true)}
											title={`${t.cropRotate} — ${t.cropRotateTip}`}
											aria-label={t.cropRotate}
										>
											<IconCrop />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={showHeat}
											onClick={() => setShowHeat(h => !h)}
											title={t.heatmapNode}
											aria-label={t.heatmapNode}
										>
											<IconHeatmap />
										</button>
										<button
											className="tb-icon-btn"
											aria-pressed={checker}
											onClick={() => setChecker(c => !c)}
											title={t.checkerBg}
											aria-label={t.checkerBg}
										>
											<IconChecker />
										</button>
									</div>
								</div>

								{tool === "pipet" && <div className="hint">{t.pipetHint}</div>}
								{tool === "selectPath" && !selectedPathId && (
									<div className="row between">
										<span className="hint">{t.pathSelectHint}</span>
										{pathEdits.length > 0 && (
											<button className="small" onClick={resetAllPathEdits}>
												{t.pathResetEdits} ({pathEdits.length})
											</button>
										)}
									</div>
								)}
								{tool === "selectPath" && selectedPathId && (
									<div className="path-bar" role="region" aria-label={t.pathSelectedLabel}>
										<span>{t.pathSelectedLabel}: <code>{selectedPathId}</code></span>
										<button className="small danger" onClick={deleteSelectedPath}>{t.pathDeleteBtn}</button>
										<span className="muted">{t.pathRecolorLabel}</span>
										<div className="recolor-row">
											{currentPalette().map(hex => (
												<button
													key={hex}
													className="recolor-chip"
													style={{ background: hex }}
													title={`${t.pathRecolorLabel} ${hex}`}
													aria-label={`${t.pathRecolorLabel} ${hex}`}
													onClick={() => recolorSelectedPath(hex)}
												/>
											))}
										</div>
										{pathEdits.length > 0 && (
											<button className="small" onClick={resetAllPathEdits}>
												{t.pathResetEdits} ({pathEdits.length})
											</button>
										)}
									</div>
								)}
								{tool === "node" && !selectedPathId && (
									<div className="row between">
										<span className="hint">{t.nodeHint}</span>
										{nodeEdits.length > 0 && (
											<button className="small" onClick={resetNodeEdits} title={t.nodeResetTip}>
												{t.nodeResetBtn} ({nodeEdits.length})
											</button>
										)}
									</div>
								)}
								{tool === "node" && selectedPathId && (
									<div className="path-bar node-bar" role="region" aria-label={t.toolNode} data-testid="node-bar">
										<span>
											<code>{selectedPathId}</code> · {t.nodeInfoPrefix}: <b data-testid="node-count">{nodeGeo?.totalNodes ?? 0}</b>
										</span>
										<span className="muted">
											{(() => {
												const n = nodeSel ? nodeGeo?.subpaths.find(s => s.subpathIndex === nodeSel.sub)?.nodes.find(x => x.index === nodeSel.idx) : null;
												return n ? `${t.nodeSelectedLabel}: #${n.index} · ${n.isSmooth ? t.nodeKindSmooth : t.nodeKindCorner}` : t.nodeNoneSelected;
											})()}
										</span>
										<label className="inline-check">
											<input type="checkbox" checked={showAllHandles} onChange={e => setShowAllHandles(e.target.checked)} />
											{t.nodeShowAllHandles}
										</label>
										<button className="small" onClick={simplifySelectedPath} title={t.nodeSimplifyTip} data-testid="node-simplify">{t.nodeSimplifyBtn}</button>
										<button className="small" onClick={resetNodeEdits} disabled={nodeEdits.length === 0} title={t.nodeResetTip} data-testid="node-reset">
											{t.nodeResetBtn}{nodeEdits.length > 0 ? ` (${nodeEdits.length})` : ""}
										</button>
									</div>
								)}
								{painting && (
									<div className="hint">
										{t.brushHintPrefix} {tool === "detail" ? t.brushHintDetail : t.brushHintSmooth}. {t.brushHintSuffix}
									</div>
								)}

								<div className="compare-wrap">
									<div
										className={`compare ${canvasBgClass}`}
										style={{
											aspectRatio: `${result.width} / ${result.height}`,
											width: `min(100%, calc((100vh - 156px) * ${(result.width / Math.max(1, result.height)).toFixed(4)}))`,
										}}
									>
										{tool === "selectPath" || tool === "node" ? (
											<div
												className="layer vec interactive-svg"
												onClick={onInteractiveSvgClick}
												dangerouslySetInnerHTML={{ __html: previewSvg }}
											/>
										) : (
											<>
												<img className="layer" src={result.original} alt={t.tagOriginal} draggable={false} style={{ clipPath: `inset(0 ${100 - split}% 0 0)` }} />
												<img className="layer vec" src={svgUrl} alt={t.tagVector} draggable={false} style={{ clipPath: `inset(0 0 0 ${split}%)` }} />
												<div className="divider" style={{ left: `${split}%` }} aria-hidden="true" />
											</>
										)}
										{showText && result.textRegions.map((r, i) => (
											<div key={i} className="textbox" aria-hidden="true" style={{ left: `${(r.x / result.width) * 100}%`, top: `${(r.y / result.height) * 100}%`, width: `${(r.w / result.width) * 100}%`, height: `${(r.h / result.height) * 100}%` }} />
										))}
										<svg
											ref={overlayRef}
											className="overlay"
											viewBox={`0 0 ${result.width} ${result.height}`}
											preserveAspectRatio="none"
											aria-hidden="true"
											data-testid="overlay"
											style={{ pointerEvents: tool === "pan" || tool === "selectPath" || tool === "node" ? "none" : "auto", cursor: tool === "pipet" ? "crosshair" : painting ? "cell" : "default" }}
											onPointerDown={onDown}
											onPointerMove={onMove}
											onPointerUp={onUp}
											onPointerCancel={onUp}
										>
											{showHeat && (() => {
												const hm = result.heatmap;
												const cell = Math.max(result.width, result.height) / 32;
												return hm.cells.map((v, i) => v > 0 ? <rect key={i} className="heatcell" x={(i % hm.cols) * cell} y={Math.floor(i / hm.cols) * cell} width={cell} height={cell} fill={heatColor(v / Math.max(1, hm.maxCell))} fillOpacity={0.25 + 0.35 * (v / Math.max(1, hm.maxCell))} /> : null);
											})()}
											{shown.map((s, i) => !s.points[0] ? null : s.points.length === 1
												? <circle key={i} cx={s.points[0][0]} cy={s.points[0][1]} r={s.radius} fill={s.kind === "detail" ? "#3b82f6" : "#f59e0b"} fillOpacity={0.4} />
												: <polyline key={i} points={s.points.map(p => `${p[0]},${p[1]}`).join(" ")} fill="none" stroke={s.kind === "detail" ? "#3b82f6" : "#f59e0b"} strokeOpacity={0.4} strokeWidth={s.radius * 2} strokeLinecap="round" strokeLinejoin="round" />)}
										</svg>
										{tool === "node" && selectedPathId && nodeGeo && (
											<NodeOverlay
												geometry={nodeGeo}
												width={result.width}
												height={result.height}
												showAllHandles={showAllHandles}
												selected={nodeSel}
												onSelect={setNodeSel}
												onMoveNode={onNodeMove}
												onMoveHandle={onNodeMoveHandle}
												onToggleSmooth={onNodeToggle}
											/>
										)}
										{tool !== "selectPath" && tool !== "node" && <span className="tag left">{t.tagOriginal}</span>}
										<span className="tag right">{outlineView ? t.tagOutline : softProof ? t.tagSoftProof : t.tagVector}</span>
										{tool !== "selectPath" && tool !== "node" && (
											<input className="slider" type="range" min={0} max={100} value={split} onChange={e => setSplit(Number(e.target.value))} aria-label={t.compareSliderAria} style={{ pointerEvents: tool === "pan" ? "auto" : "none" }} />
										)}
									</div>
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
						<div className="panel-tabs">
							<div className="seg" role="tablist" aria-label={t.panelAria}>
								<button role="tab" aria-selected={panelTab === "trace"} aria-pressed={panelTab === "trace"} onClick={() => setPanelTab("trace")}>
									{t.panelTabTrace}
								</button>
								<button role="tab" aria-selected={panelTab === "colors"} aria-pressed={panelTab === "colors"} onClick={() => setPanelTab("colors")}>
									{t.panelTabColors}
									{result ? ` (${currentPalette().length})` : ""}
								</button>
								<button role="tab" aria-selected={panelTab === "export"} aria-pressed={panelTab === "export"} onClick={() => setPanelTab("export")}>
									{t.panelTabExport}
									{exp.dieCutContour ? <span className="tab-cut-dot" aria-hidden="true" /> : null}
								</button>
							</div>
						</div>

						<div className="panel-body">
							{panelTab === "trace" && (
								<div className="panel-section-stack">
									{/* Compact 1-row Custom Presets */}
									<div className="field-block">
										<div className="field-head">
											<span className="field-title">{t.customPresets}</span>
										</div>
										<div className="preset-row" style={{ marginTop: 4 }}>
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
											<button
												className="small icon-btn"
												onClick={() => setAddingPreset(v => !v)}
												title={addingPreset ? t.close : t.addPresetBtn}
												aria-label={addingPreset ? t.close : t.addPresetBtn}
											>
												{addingPreset ? "×" : "+"}
											</button>
											{selectedPresetId && (
												<button className="small" onClick={() => void removeSelectedPreset()} aria-label={t.deletePresetAria}>
													{t.delete}
												</button>
											)}
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
									</div>

									{/* Production Mode Dropdown */}
									<div className="field-block">
										<label className="field-label" htmlFor="prodMode">
											<span>{t.productionModeTitle}</span>
										</label>
										<select
											id="prodMode"
											aria-label={t.productionModeAria}
											value={opts.productionMode}
											onChange={e => switchProductionMode(e.target.value as ProductionMode)}
										>
											<option value="standard">{t.modeStandard}</option>
											<option value="screenPrint">{t.modeScreenPrint}</option>
											<option value="cutEmbroidery">{t.modeCutEmbroidery}</option>
											<option value="shading">{t.modeShading}</option>
											<option value="centerline">{t.modeCenterline}</option>
										</select>

										{opts.productionMode === "screenPrint" && (
											<div className="mode-box">
												<label className="field-label" style={{ marginTop: 0 }}>
													<span>{t.sablonMethodLabel}</span>
													<TipDot text={opts.sablonKnockout ? t.sablonKnockoutHint : t.sablonHint} />
												</label>
												<div className="seg seg2" role="group" aria-label={t.sablonMethodLabel} style={{ marginTop: 4 }}>
													<button aria-pressed={!opts.sablonKnockout} onClick={() => set({ sablonKnockout: false }, t.sablonStacking)}>{t.sablonStacking}</button>
													<button aria-pressed={opts.sablonKnockout} onClick={() => set({ sablonKnockout: true }, t.sablonKnockout)}>{t.sablonKnockout}</button>
												</div>
												<label className="field-label" htmlFor="minStroke">{t.minStrokeLabel}</label>
												<input id="minStroke" className="num" type="number" min={0} max={6} step={0.25} value={opts.minStrokePt} onChange={e => set({ minStrokePt: Math.max(0, Number(e.target.value)) }, t.minStrokeLabel)} />
												<label className="check"><input type="checkbox" checked={opts.registrationMarks} onChange={e => { const v = e.target.checked; set({ registrationMarks: v }, t.regMarksLabel); setExp(ex => ({ ...ex, registrationMarks: v })); }} /> {t.regMarksLabel}</label>
												{opts.colors > 6 && <div className="warn">{t.sablonOver6Warn}</div>}
											</div>
										)}

										{opts.productionMode === "cutEmbroidery" && (
											<div className="mode-box">
												<label className="check" style={{ marginTop: 0 }}>
													<input type="checkbox" checked={opts.stripBackground} onChange={e => { const v = e.target.checked; set({ stripBackground: v }, t.stripBgLabel); setExp(ex => ({ ...ex, stripBackground: v })); setChecker(v); }} />
													<span>{t.stripBgLabel}</span>
													<TipDot text={t.cutHint} />
												</label>
												<label className="field-label" htmlFor="minStrokeCut">{t.minStrokeLabel}</label>
												<input id="minStrokeCut" className="num" type="number" min={0} max={6} step={0.25} value={opts.minStrokePt} onChange={e => set({ minStrokePt: Math.max(0, Number(e.target.value)) }, t.minStrokeLabel)} />
											</div>
										)}

										{opts.productionMode === "shading" && (
											<div className="mode-box">
												<div className="hint">{t.shadingHint}</div>
											</div>
										)}

										{opts.productionMode === "centerline" && (
											<div className="mode-box">
												<label className="check" style={{ marginTop: 0 }}>
													<input type="checkbox" checked={opts.centerlineInvert} onChange={e => set({ centerlineInvert: e.target.checked }, t.centerlineInvertLabel)} />
													<span>{t.centerlineInvertLabel}</span>
													<TipDot text={t.centerlineHint} />
												</label>
												<label className="field-label" htmlFor="centerlineStroke">
													<span>{t.centerlineStrokeLabel}</span>
													<b>{opts.centerlineStrokePt.toFixed(2)} pt</b>
												</label>
												<input
													id="centerlineStroke"
													type="range"
													min={0.25}
													max={8}
													step={0.25}
													value={opts.centerlineStrokePt}
													onChange={e => set({ centerlineStrokePt: Number(e.target.value) }, t.centerlineStrokeLabel)}
												/>
											</div>
										)}
									</div>

									{/* 1-Click Auto Remove Background */}
									<div className="mode-box" style={{ marginTop: 0 }}>
										<label className="check" style={{ marginTop: 0 }}>
											<input
												type="checkbox"
												checked={opts.preprocess.removeBackground}
												onChange={e => {
													const checked = e.target.checked;
													set({ preprocess: { ...opts.preprocess, removeBackground: checked } }, t.removeBgToggle);
													if (checked) setChecker(true);
												}}
											/>
											<span style={{ flex: 1 }}><b>{t.removeBgToggle}</b></span>
											<TipDot text={t.removeBgHint} />
										</label>
										{opts.preprocess.removeBackground && (
											<>
												<label className="field-label" htmlFor="bgTol">
													<span>{t.bgToleranceLabel}</span>
													<b>{opts.preprocess.bgTolerance}</b>
												</label>
												<input
													id="bgTol"
													type="range"
													min={0}
													max={100}
													value={opts.preprocess.bgTolerance}
													onChange={e => set({ preprocess: { ...opts.preprocess, bgTolerance: Number(e.target.value) } }, t.bgToleranceLabel)}
												/>
												<label className="field-label" htmlFor="bgDefringe">
													<span>{t.defringeLabel}</span>
													<b>{opts.preprocess.defringePx} px</b>
												</label>
												<input
													id="bgDefringe"
													type="range"
													min={0}
													max={3}
													value={opts.preprocess.defringePx}
													onChange={e => set({ preprocess: { ...opts.preprocess, defringePx: Number(e.target.value) } }, t.defringeLabel)}
												/>
											</>
										)}
									</div>

									{(opts.preprocess.crop !== null || Math.abs(opts.preprocess.rotateDeg) > 0.05) && (
										<div className="mode-box row between" style={{ marginTop: 0 }}>
											<span className="hint" style={{ color: "var(--ink)" }}>
												{t.preprocessActive}
												{Math.abs(opts.preprocess.rotateDeg) > 0.05 ? ` (${opts.preprocess.rotateDeg}°)` : ""}
												{opts.preprocess.crop ? ` · ${opts.preprocess.crop[2]}×${opts.preprocess.crop[3]}` : ""}
											</span>
											<button className="small" onClick={resetPreprocess}>{t.preprocessReset}</button>
										</div>
									)}

									{/* Core Trace Controls (clean tooltips, no verbose paragraphs) */}
									<div className="field-block">
										{opts.productionMode !== "centerline" && (
											<>
												<label className="field-label" htmlFor="colors">
													<span>{t.colorCount}</span>
													<b>{Math.min(maxColors, Math.max(minColors, opts.colors))}</b>
												</label>
												<input id="colors" type="range" min={minColors} max={maxColors} value={Math.min(maxColors, Math.max(minColors, opts.colors))} onChange={e => set({ colors: Number(e.target.value) }, t.colorCount)} />
											</>
										)}
										<label className="field-label" htmlFor="smooth">
											<span className="label-with-tip">{t.smoothness} <TipDot text={t.smoothHint} /></span>
											<b>{opts.smooth < 34 ? t.smoothDetail : opts.smooth > 66 ? t.smoothSmooth : t.smoothMedium}</b>
										</label>
										<input id="smooth" type="range" min={0} max={100} value={opts.smooth} onChange={e => set({ smooth: Number(e.target.value) }, t.smoothness)} />
										<label className="field-label" htmlFor="target">
											<span className="label-with-tip">{t.targetNodesLabel} <TipDot text={t.targetHint} /></span>
										</label>
										<select id="target" value={targetChoice} onChange={e => { recordHistory(t.targetNodesLabel); setTargetChoice(e.target.value as TargetChoice); }}>
											<option value="auto">{t.targetAuto}</option>
											<option value="500">500</option>
											<option value="1000">1000</option>
											<option value="2000">2000</option>
											<option value="custom">{t.targetCustom}</option>
										</select>
										{targetChoice === "custom" && <input className="num" type="number" min={1} step={50} value={customTarget} onChange={e => setCustomTarget(e.target.value)} aria-label={t.targetNodesLabel} />}
									</div>

									{/* Collapsed-by-default Advanced Accordion */}
									<details className="adv-details">
										<summary>{t.advancedSection}</summary>
										<div className="adv-body">
											{opts.productionMode !== "centerline" && (
												<>
													<label className="check" style={{ marginTop: 0 }}>
														<input type="checkbox" checked={opts.keepLines} onChange={e => set({ keepLines: e.target.checked }, t.keepLines)} />
														<span>{t.keepLines}</span>
													</label>
													<label className="check">
														<input type="checkbox" checked={opts.detectPrimitives} onChange={e => set({ detectPrimitives: e.target.checked }, t.detectPrimitives)} />
														<span style={{ flex: 1 }}>{t.detectPrimitives}</span>
														<TipDot text={t.primitivesHint} />
													</label>
													<label className="field-label" htmlFor="symmetry">
														<span className="label-with-tip">{t.symmetryLabel} <TipDot text={t.symmetryHint} /></span>
													</label>
													<select id="symmetry" value={opts.symmetry} onChange={e => set({ symmetry: e.target.value as Options["symmetry"] }, t.symmetryLabel)}>
														<option value="off">{t.symOff}</option>
														<option value="auto">{t.symAuto}</option>
														<option value="vertical">{t.symVert}</option>
														<option value="horizontal">{t.symHoriz}</option>
													</select>
												</>
											)}
											<label className="check">
												<input type="checkbox" checked={!appSettings.preferMedianOnly && opts.aiClean} disabled={appSettings.preferMedianOnly} onChange={e => set({ aiClean: e.target.checked }, t.aiCleanLabel)} />
												<span style={{ flex: 1 }}>{t.aiCleanLabel}</span>
												<TipDot text={appSettings.preferMedianOnly ? t.medianOnlyHint : t.aiCleanHint} />
											</label>
											{opts.aiClean && !hasSidecar && !appSettings.preferMedianOnly && <div className="warn">{t.noSidecarWarn}</div>}
										</div>
									</details>

									{result && result.textRegions.length > 0 && (
										<div className="warn" role="status" id="text-warning">
											{t.textWarnPrefix}{result.textRegions.length}{t.textWarnSuffix}
											<div className="row" style={{ marginTop: 6 }}>
												<button className="small" aria-pressed={showText} onClick={() => setShowText(s => !s)}>{showText ? t.hideTextAreas : t.highlightTextAreas}</button>
											</div>
										</div>
									)}
									{result?.notes.map(n => <div className="warn" key={n}>{n}</div>)}
								</div>
							)}

							{panelTab === "colors" && (
								<div className="panel-section-stack">
									{!result ? (
										<div className="mode-box muted" style={{ textAlign: "center", padding: "20px 12px" }}>
											{t.noImageTabHint}
										</div>
									) : (
										<>
											<div className="palette-head" style={{ marginTop: 0 }}>
												<span className="field-title">{t.paletteTitle}</span>
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
													const upper = c.toUpperCase();
													const isHidden = hiddenLayers.has(c);
													const isOut = outOfGamutSet.has(upper);
													const spot = spotMap.get(upper) ?? "";
													const isInspected = inspectedColor === c;
													const codes = isInspected ? formatColorCodes(c) : null;
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
																<button
																	className={`chip-btn ${isInspected ? "active" : ""}`}
																	style={{ background: softProof ? (proofMap.get(upper) ?? c) : c }}
																	onClick={() => {
																		setMergeSource(null);
																		setInspectedColor(cur => (cur === c ? null : c));
																	}}
																	title={t.inspectColorTip}
																	aria-label={`${t.inspectColorTip} ${c}`}
																	aria-expanded={isInspected}
																/>
																<code>{c}</code>
																{isOut && <span className="gamut-warn" title={t.outOfGamutTip} aria-label={t.outOfGamutTip}>⚠</span>}
																{spot && <span className="spot-pill" title={`${t.spotBadgePrefix} ${spot}`}>{spot}</span>}
																<button
																	className="small"
																	aria-expanded={mergeSource === c}
																	onClick={() => {
																		setInspectedColor(null);
																		setMergeSource(s => (s === c ? null : c));
																	}}
																	disabled={currentPalette().length <= 2}
																	aria-label={`${t.mergeBtn} ${c}`}
																>
																	{t.mergeBtn}
																</button>
																<button className="small" onClick={() => removeColor(c)} disabled={currentPalette().length <= 2} aria-label={`${t.removeBtn} ${c}`}>{t.removeBtn}</button>
															</div>

															{isInspected && codes && (
																<div className="color-inspector" role="region" aria-label={`Color Inspector ${c}`}>
																	<div className="ci-grid">
																		{(["hex", "rgb", "cmyk", "hsl"] as const).map(fmt => {
																			const k = `${c}-${fmt}`;
																			const val = codes[fmt];
																			return (
																				<div key={fmt} style={{ display: "contents" }}>
																					<span className="ci-label">{fmt.toUpperCase()}</span>
																					<span className="ci-val" title={val}>{val}</span>
																					<button className="small" onClick={() => void copyCodeValue(k, val)}>
																						{copiedKey === k ? t.copiedTag : t.copyBtn}
																					</button>
																				</div>
																			);
																		})}
																	</div>
																	<div className="ci-spot">
																		<span className="ci-label">{t.spotInkLabel}</span>
																		<div className="ci-spot-row">
																			<input
																				className="num"
																				type="text"
																				value={spot}
																				placeholder={t.spotInkPlaceholder}
																				aria-label={t.spotInkLabel}
																				onChange={e => setSpotName(c, e.target.value)}
																			/>
																			{spot && (
																				<button className="small" onClick={() => setSpotName(c, "")}>{t.spotInkClear}</button>
																			)}
																		</div>
																	</div>
																</div>
															)}

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
												{hasCutLayer && (
													<li className={`swatch-item ${hiddenLayers.has(CUT_LAYER) ? "layer-hidden" : ""}`}>
														<div className="swatch">
															<button
																className="small icon-btn"
																aria-pressed={hiddenLayers.has(CUT_LAYER)}
																onClick={() => toggleLayer(CUT_LAYER)}
																title={hiddenLayers.has(CUT_LAYER) ? t.toggleLayerShow : t.toggleLayerHide}
																aria-label={t.layerCutContour}
															>
																<EyeIcon hidden={hiddenLayers.has(CUT_LAYER)} />
															</button>
															<span className="chip chip-cut" aria-hidden="true" />
															<span className="layer-label">{t.layerCutContour} ({exp.dieCutSpotName || "CutContour"})</span>
														</div>
													</li>
												)}
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

											{/* Saved Palette Library + .ase/.gpl/.json/.css export */}
											<div className="mode-box" style={{ marginTop: 4 }}>
												<div className="row between">
													<strong style={{ fontSize: 12 }}>{t.paletteLibTitle}</strong>
													<button className="small" onClick={() => setAddingSavedPalette(v => !v)}>
														{addingSavedPalette ? t.close : t.savePaletteBtn}
													</button>
												</div>
												{addingSavedPalette && (
													<div className="preset-add">
														<input
															className="num"
															type="text"
															placeholder={t.paletteNamePlaceholder}
															value={savedPaletteName}
															onChange={e => setSavedPaletteName(e.target.value)}
															onKeyDown={e => { if (e.key === "Enter") void saveCurrentToPaletteLibrary(); }}
														/>
														<button className="small" onClick={() => void saveCurrentToPaletteLibrary()} disabled={!savedPaletteName.trim()}>{t.save}</button>
													</div>
												)}
												{savedPalettes.length > 0 && (
													<div className="preset-row">
														<select
															aria-label={t.paletteLibTitle}
															value={selectedSavedPaletteId}
															onChange={e => {
																const sp = savedPalettes.find(x => x.id === e.target.value);
																if (sp) applySavedPalette(sp);
															}}
														>
															<option value="">{t.chooseSavedPalette}</option>
															{savedPalettes.map(sp => (
																<option key={sp.id} value={sp.id}>{sp.name} ({sp.colors.length})</option>
															))}
														</select>
														<button className="small" onClick={() => void removeSelectedSavedPalette()} disabled={!selectedSavedPaletteId}>{t.delete}</button>
													</div>
												)}
												<div className="row" style={{ marginTop: 6 }}>
													<button className="small" onClick={() => void exportAs("ase")}>.ase</button>
													<button className="small" onClick={() => void exportPaletteText("gpl")}>{t.exportGpl}</button>
													<button className="small" onClick={() => void exportPaletteText("json")}>{t.exportJsonPal}</button>
													<button className="small" onClick={() => void exportPaletteText("css")}>{t.exportCssPal}</button>
												</div>
											</div>
											<div className="hint">{paletteDirty ? t.paletteDirtyHint : palette !== null ? t.paletteManualHint : t.paletteAutoHint}</div>
										</>
									)}
								</div>
							)}

							{panelTab === "export" && (
								<div className="panel-section-stack">
									{!result ? (
										<div className="mode-box muted" style={{ textAlign: "center", padding: "20px 12px" }}>
											{t.noImageTabHint}
										</div>
									) : (
										<>
											<div className="field-block">
												<label className="field-label" style={{ marginTop: 0 }}>
													<span className="label-with-tip">{t.colorSpaceLabel} <TipDot text={t.colorSpaceHintCmyk} /></span>
												</label>
												<div className="seg seg2" role="group" aria-label={t.colorSpaceLabel} style={{ marginTop: 4 }}>
													<button aria-pressed={exp.colorSpace === "rgb"} onClick={() => setExp(ex => ({ ...ex, colorSpace: "rgb" as ColorSpace }))}>RGB</button>
													<button aria-pressed={exp.colorSpace === "cmyk"} onClick={() => setExp(ex => ({ ...ex, colorSpace: "cmyk" as ColorSpace }))}>CMYK</button>
												</div>
											</div>

											{/* Die-Cut / Kiss-Cut Contour */}
											<div className="mode-box" style={{ marginTop: 0 }}>
												<label className="check" style={{ marginTop: 0 }}>
													<input
														type="checkbox"
														checked={exp.dieCutContour}
														onChange={e => setExp(ex => ({ ...ex, dieCutContour: e.target.checked }))}
													/>
													<span style={{ flex: 1 }}><b>{t.dieCutEnable}</b></span>
													<TipDot text={t.dieCutHint} />
												</label>
												{exp.dieCutContour && (
													<>
														<label className="field-label" htmlFor="dieCutOffset">
															<span>{t.dieCutOffsetLabel}</span>
															<b>{exp.dieCutOffsetMm.toFixed(1)} mm</b>
														</label>
														<input
															id="dieCutOffset"
															type="range"
															min={0.5}
															max={10}
															step={0.5}
															value={exp.dieCutOffsetMm}
															onChange={e => setExp(ex => ({ ...ex, dieCutOffsetMm: Number(e.target.value) }))}
														/>
														<label className="field-label" htmlFor="dieCutSpot">{t.dieCutSpotNameLabel}</label>
														<input
															id="dieCutSpot"
															className="num"
															type="text"
															value={exp.dieCutSpotName}
															placeholder="CutContour"
															onChange={e => setExp(ex => ({ ...ex, dieCutSpotName: e.target.value }))}
														/>
														<label className="check">
															<input
																type="checkbox"
																checked={exp.dieCutBridgeIslands}
																onChange={e => setExp(ex => ({ ...ex, dieCutBridgeIslands: e.target.checked }))}
															/>
															{t.dieCutBridgeLabel}
														</label>
														<label className="check" style={{ marginTop: 8 }}>
															<input
																type="checkbox"
																checked={exp.stickerBacking}
																onChange={e => setExp(ex => ({ ...ex, stickerBacking: e.target.checked }))}
															/>
															<span style={{ flex: 1 }}>{t.stickerBackingEnable}</span>
															<TipDot text={t.stickerBackingHint} />
														</label>
														{exp.stickerBacking && (
															<>
																<div className="row between" style={{ marginTop: 6 }}>
																	<label className="field-label" htmlFor="stickerBackingColor" style={{ marginTop: 0 }}>{t.stickerBackingColorLabel}</label>
																	<div className="row" style={{ gap: 6 }}>
																		<input
																			id="stickerBackingColor"
																			type="color"
																			value={exp.stickerBackingColor}
																			onChange={e => setExp(ex => ({ ...ex, stickerBackingColor: e.target.value.toUpperCase() }))}
																			style={{ width: 28, height: 24, padding: 0, border: "none", background: "transparent", cursor: "pointer" }}
																		/>
																		<code style={{ fontSize: 11 }}>{exp.stickerBackingColor}</code>
																	</div>
																</div>
																<label className="field-label" htmlFor="stickerBleed">
																	<span>{t.stickerBleedLabel}</span>
																	<b>{exp.stickerBleedMm.toFixed(1)} mm</b>
																</label>
																<input
																	id="stickerBleed"
																	type="range"
																	min={0}
																	max={3}
																	step={0.5}
																	value={exp.stickerBleedMm}
																	onChange={e => setExp(ex => ({ ...ex, stickerBleedMm: Number(e.target.value) }))}
																/>
															</>
														)}
													</>
												)}
											</div>

											{/* Compact 2-col DXF controls */}
											<div className="two-col">
												<div>
													<label className="field-label" htmlFor="dxfUnit" style={{ marginTop: 0 }}>{t.dxfUnitLabel}</label>
													<select id="dxfUnit" value={exp.dxfUnit} onChange={e => setExp({ ...exp, dxfUnit: e.target.value as DxfUnit })}>
														<option value="mm">mm ({appSettings.defaultDpi} DPI)</option>
														<option value="px">px</option>
													</select>
												</div>
												<div>
													<label className="field-label" htmlFor="dxfTargetWidth" style={{ marginTop: 0 }}>{t.dxfTargetWidthLabel}</label>
													<input
														id="dxfTargetWidth"
														className="num"
														style={{ marginTop: 4 }}
														type="number"
														min={1}
														step={1}
														disabled={exp.dxfUnit !== "mm"}
														placeholder={`${t.dxfTargetWidthPlaceholder}`}
														value={dxfWidthInput}
														onChange={e => setDxfWidthInput(e.target.value)}
													/>
												</div>
											</div>

											{/* Compact 2-col Lines & Trap controls */}
											<div className="two-col">
												<div>
													<label className="field-label" htmlFor="linesAs" style={{ marginTop: 0 }}>{t.linesAsLabel}</label>
													<select id="linesAs" value={opts.productionMode === "cutEmbroidery" ? "shape" : opts.productionMode === "centerline" ? "stroke" : exp.linesAs} disabled={opts.productionMode === "cutEmbroidery" || opts.productionMode === "centerline"} onChange={e => setExp({ ...exp, linesAs: e.target.value as ExportOptions["linesAs"] })}>
														<option value="stroke">{t.linesStroke}</option>
														<option value="shape">{t.linesShape}</option>
													</select>
												</div>
												<div>
													<label className="field-label" htmlFor="trap" style={{ marginTop: 0 }}>{t.trapLabel}</label>
													<select id="trap" value={opts.productionMode === "cutEmbroidery" || opts.productionMode === "centerline" ? 0 : exp.trapPt} disabled={opts.productionMode === "cutEmbroidery" || opts.productionMode === "centerline"} onChange={e => setExp({ ...exp, trapPt: Number(e.target.value) })}>
														<option value={0}>{t.trapNone}</option>
														<option value={0.25}>0,25 pt</option>
														<option value={0.5}>0,5 pt</option>
														<option value={1}>1 pt</option>
														<option value={1.5}>1,5 pt</option>
														<option value={2}>2 pt</option>
													</select>
												</div>
											</div>

											<label className="check" style={{ marginTop: 0 }}>
												<input type="checkbox" checked={withAse} onChange={e => setWithAse(e.target.checked)} />
												<span>{t.includeAse}</span>
											</label>

											{/* 2-Column Export Action Grid */}
											<div className="export-grid">
												<button onClick={() => void doCopySvg()} title={`${t.copySvgTip} (${activeShortcuts.copySvg})`}>{t.copySvgBtn}</button>
												<button onClick={() => void exportAs("svg")}>{t.exportSvg}</button>
												<button onClick={() => void exportAs("pdf")}>{t.exportPdf}{exp.colorSpace === "cmyk" ? " (CMYK)" : ""}</button>
												<button onClick={() => void exportAs("dxf")}>{t.exportDxf}</button>
												<button onClick={() => void exportAs("eps")}>{t.exportEps}{exp.colorSpace === "cmyk" ? " (CMYK)" : ""}</button>
												<button onClick={() => void exportAs("ase")}>{t.exportAse}{exp.colorSpace === "cmyk" ? " (CMYK)" : ""}</button>
											</div>

											{opts.productionMode === "screenPrint" && (
												<div className="sep-box" style={{ marginTop: 4 }}>
													<label className="check" style={{ marginTop: 0 }}><input type="checkbox" checked={sepPdf} onChange={e => setSepPdf(e.target.checked)} /> {t.includeSepPdf}</label>
													<button style={{ width: "100%", marginTop: 6 }} onClick={() => void exportSeparationFilms()}>{t.exportSepBtn}</button>
												</div>
											)}
										</>
									)}
								</div>
							)}
						</div>

						{/* Sticky Panel Footer — always visible across all 3 tabs */}
						<div className="panel-footer">
							{result && (
								<div
									className="footer-stats"
									title={`${t.statPrimitives}: ${result.primitives} · ${t.statMae}: ${result.mae === null ? "-" : result.mae.toFixed(2)} · ${t.statTime}: ${result.secs.toFixed(1)} ${t.secUnit} (${result.usedAi ? "Real-ESRGAN" : "median"})`}
								>
									<span><b>{result.paths}</b> {t.statPaths.toLowerCase()}</span>
									<span>·</span>
									<span><b>{result.nodes.toLocaleString(locale)}</b> {t.statNodes.toLowerCase()}</span>
									<span>·</span>
									<span><b>{fmtKb(result.bytes)}</b></span>
									{result.mae !== null && (
										<>
											<span>·</span>
											<span>MAE <b>{result.mae.toFixed(2)}</b></span>
										</>
									)}
								</div>
							)}
							{dirty && (
								<div className="pending" role="status" style={{ marginBottom: 4 }}>
									{t.pendingChanges}
									{palette !== null || strokes.length > 0 || hasPreprocess || pathEdits.length > 0 || nodeEdits.length > 0
										? ` (${[
												palette !== null ? t.pendingPalette : "",
												strokes.length > 0 ? `${strokes.length} ${t.pendingStrokesSuffix}` : "",
												hasPreprocess ? t.pendingPreprocess : "",
												pathEdits.length > 0 ? `${pathEdits.length} ${t.pendingPathEdits}` : "",
												nodeEdits.length > 0 ? `${nodeEdits.length} ${t.pendingNodeEdits}` : "",
											]
												.filter(Boolean)
												.join(", ")})`
										: ""}.
								</div>
							)}
							<button className={`primary footer-apply ${dirty ? "dirty" : ""}`} onClick={apply} disabled={!path || busy}>
								{result ? t.applyAgain : t.vectorizeBtn}
							</button>
							{error && <div className="error footer-msg" role="alert">{error}</div>}
							{info && <div className="ok footer-msg" role="status">{info}</div>}
						</div>
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

			{settingsModal !== null && (
				<SettingsModal
					settings={appSettings}
					initialTab={settingsModal.tab}
					onUpdate={onUpdateSettings}
					onApplyExportDefaultsNow={next => {
						setExp(ex => ({
							...ex,
							colorSpace: next.defaultColorSpace,
							dxfUnit: next.defaultDxfUnit,
							dieCutOffsetMm: next.defaultDieCutOffsetMm,
							dieCutSpotName: next.defaultDieCutSpotName,
							dieCutBridgeIslands: next.defaultDieCutBridge,
						}));
					}}
					onClose={() => setSettingsModal(null)}
				/>
			)}
		</div>
	);
}

export default function App() {
	const [settings, setSettings] = useState<AppSettings>(DEFAULT_APP_SETTINGS);

	useEffect(() => {
		void getAppSettings()
			.then(s => {
				setSettings({ ...DEFAULT_APP_SETTINGS, ...s });
			})
			.catch(() => {});
	}, []);

	// Apply light/dark/system theme to <html data-theme="...">
	useEffect(() => {
		const applyTheme = () => {
			let resolved: "dark" | "light" = "dark";
			if (settings.theme === "light") resolved = "light";
			else if (settings.theme === "dark") resolved = "dark";
			else resolved = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
			document.documentElement.setAttribute("data-theme", resolved);
		};
		applyTheme();
		if (settings.theme !== "system") return;
		const mq = window.matchMedia("(prefers-color-scheme: light)");
		mq.addEventListener("change", applyTheme);
		return () => mq.removeEventListener("change", applyTheme);
	}, [settings.theme]);

	const updateSettings = useCallback((next: AppSettings) => {
		setSettings(next);
		void saveAppSettings(next)
			.then(saved => setSettings({ ...DEFAULT_APP_SETTINGS, ...saved }))
			.catch(() => {});
	}, []);

	const setLang = useCallback(
		(nextLang: Lang) => {
			setSettings(prev => {
				const updated: AppSettings = { ...prev, language: nextLang };
				void saveAppSettings(updated).catch(() => {});
				return updated;
			});
		},
		[],
	);

	return (
		<I18nProvider lang={settings.language} setLang={setLang}>
			<TraseaApp appSettings={settings} onUpdateSettings={updateSettings} />
		</I18nProvider>
	);
}
