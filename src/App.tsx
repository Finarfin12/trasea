import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import BatchView from "./BatchView";
import Licenses from "./Licenses";
import {
	collectDroppedImages,
	deletePreset,
	getLicenseStatus,
	getSampleImagePath,
	licenses,
	listPresets,
	loadProjectFile,
	onDrop,
	onProgress,
	pickColor,
	pickFolder,
	pickImage,
	pickImageOrProject,
	pickProjectSavePath,
	pickSavePath,
	processImage,
	saveFile,
	savePreset,
	saveProjectFile,
	saveSeparations,
	sidecarAvailable,
	type BrushStroke,
	type ExportKind,
	type ExportOptions,
	type LicenseStatus,
	type Options,
	type Processed,
	type ProductionMode,
	type UserPreset,
} from "./api";
import { emptyHistory, pushSnapshot, stepRedo, stepUndo, type EditorSnapshot, type HistoryState, type TargetChoice } from "./history";

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
};

const DEFAULT_EXP: ExportOptions = { linesAs: "stroke", trapPt: 0, registrationMarks: false, stripBackground: false };

type View = "editor" | "batch";
type Tool = "pan" | "detail" | "smooth" | "pipet";

const baseName = (p: string): string => (p.split(/[\\/]/).pop() ?? "gambar").replace(/\.[^.]+$/, "");
const fmtKb = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : "Terjadi kesalahan yang tidak dikenal.");
const rgb = (hex: string): [number, number, number] => [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
const heatColor = (r: number): string => `hsl(${Math.round(120 * (1 - Math.min(1, Math.max(0, r))))}, 90%, 50%)`;

export default function App() {
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
	const [withAse, setWithAse] = useState(true);
	const [sepPdf, setSepPdf] = useState(false);
	const [checker, setChecker] = useState(false);
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
			opts: { ...opts },
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
		setOpts(snap.opts);
		setPalette(snap.palette);
		setPaletteMerges(snap.paletteMerges);
		setStrokes(snap.strokes);
		setTargetChoice(snap.targetChoice);
		setCustomTarget(snap.customTarget);
		setExp(snap.exp);
		setChecker(snap.opts.productionMode === "cutEmbroidery" && snap.opts.stripBackground);
		setMergeSource(null);
	}, []);

	const doUndo = useCallback(() => {
		const res = stepUndo(history, makeSnapshot("Keadaan sebelumnya"));
		if (!res) return;
		setHistory(res.history);
		restoreSnapshot(res.target);
		setInfo(`Undo: ${res.target.label}`);
	}, [history, makeSnapshot, restoreSnapshot]);

	const doRedo = useCallback(() => {
		const res = stepRedo(history, makeSnapshot("Ulangi"));
		if (!res) return;
		setHistory(res.history);
		restoreSnapshot(res.target);
		setInfo(`Redo: ${res.target.label}`);
	}, [history, makeSnapshot, restoreSnapshot]);

	const signature = useMemo(
		() => JSON.stringify({ ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes }),
		[opts, palette, paletteMerges, targetNodes, strokes],
	);

	const openPrivacyModal = useCallback(async (tab: "license" | "privacy" | "thirdParty" = "license", hint = "") => {
		const md = await licenses();
		setModalOpen({ markdown: md, tab, hint });
	}, []);

	const run = useCallback(async (p: string, o: Options) => {
		if (busyRef.current) return;
		busyRef.current = true;
		setBusy(true);
		setError("");
		setInfo("");
		setStage("Memuat gambar");
		setFraction(0);
		setPath(p);
		try {
			const r = await processImage(p, o);
			setResult(r);
			setAppliedSig(JSON.stringify(o));
		} catch (e) {
			setError(errText(e));
		} finally {
			busyRef.current = false;
			setBusy(false);
		}
	}, []);

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
			setHistory(emptyHistory());
			setTool("pan");
			void run(p, { ...optsRef.current, customPalette: null, paletteMerges: [], brushStrokes: [] });
		},
		[run],
	);

	const openProject = useCallback(
		async (projFile: string) => {
			setError("");
			setInfo("");
			try {
				const loaded = await loadProjectFile(projFile);
				const po = loaded.project.options;
				const pe = loaded.project.exportOptions;
				setView("editor");
				setProjectPath(projFile);
				setOpts(po);
				setExp(pe);
				setPalette(po.customPalette);
				setPaletteMerges(po.paletteMerges ?? []);
				setStrokes(po.brushStrokes ?? []);
				setChecker(po.productionMode === "cutEmbroidery" && po.stripBackground);
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
					if (!loaded.hashMatches) {
						setInfo("Proyek dibuka. Catatan: gambar sumber berubah sejak proyek disimpan; pengaturan & sapuan kuas tetap dimuat.");
					} else {
						setInfo(`Proyek dibuka: ${projFile}`);
					}
				} else {
					setPath(null);
					setResult(null);
					setMissingSourceProject(loaded.project.sourcePath);
					setInfo("Pengaturan dan sapuan kuas dari proyek sudah dimuat. Pilih lokasi baru gambar sumber untuk melanjutkan.");
				}
			} catch (e) {
				setError(errText(e));
			}
		},
		[run],
	);

	const relinkMissingSource = async () => {
		const newImg = await pickImage();
		if (!newImg) return;
		setMissingSourceProject(null);
		const mergedOpts: Options = { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes };
		await run(newImg, mergedOpts);
		setInfo(`Gambar sumber ditautkan ulang ke ${newImg}`);
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
				await saveProjectFile(dest, path, fullOpts, exp);
				setProjectPath(dest);
				setInfo(`Proyek tersimpan: ${dest}`);
			} catch (e) {
				setError(errText(e));
			}
		},
		[path, projectPath, opts, palette, paletteMerges, targetNodes, strokes, exp],
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
						setError("File atau folder itu tidak berisi gambar (.png/.jpg/.webp/.bmp) atau proyek .trasea.");
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
	}, [openPicture, openProject]);

	// Global keyboard shortcuts: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z, Ctrl+S, Ctrl+Shift+S, Ctrl+O
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
			} else if (k === "y" && !editingText && view === "editor") {
				e.preventDefault();
				doRedo();
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
	}, [view, path, doUndo, doRedo, saveProject, choose]);

	const switchProductionMode = (mode: ProductionMode) => {
		recordHistory(`Ubah Mode ${mode === "screenPrint" ? "Sablon" : mode === "cutEmbroidery" ? "Cutting" : "Standar"}`);
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
			return {
				...o,
				productionMode: "cutEmbroidery",
				colors: o.productionMode === "screenPrint" ? standardBackupRef.current.colors : o.colors,
				smooth: o.productionMode === "cutEmbroidery" ? o.smooth : 70,
				minStrokePt: o.productionMode === "cutEmbroidery" ? o.minStrokePt : 1.5,
				stripBackground: true,
				registrationMarks: false,
			};
		});
		if (mode === "standard") {
			setExp({ linesAs: "stroke", trapPt: 0, registrationMarks: false, stripBackground: false });
			setChecker(false);
		} else if (mode === "screenPrint") {
			setExp({ linesAs: "stroke", trapPt: 0.5, registrationMarks: true, stripBackground: false });
			setChecker(false);
		} else {
			setExp({ linesAs: "shape", trapPt: 0, registrationMarks: false, stripBackground: true });
			setChecker(true);
		}
	};

	const applyPreset = (preset: UserPreset) => {
		recordHistory(`Terapkan preset "${preset.name}"`);
		setSelectedPresetId(preset.id);
		if (preset.options.productionMode === "standard") {
			standardBackupRef.current = { colors: preset.options.colors, smooth: preset.options.smooth };
		}
		setOpts({ ...preset.options, customPalette: palette, paletteMerges, brushStrokes: strokes });
		setExp(preset.exportOptions);
		setChecker(preset.options.productionMode === "cutEmbroidery" && preset.options.stripBackground);
		if (preset.options.targetNodes === null) setTargetChoice("auto");
		else if (["500", "1000", "2000"].includes(String(preset.options.targetNodes))) setTargetChoice(String(preset.options.targetNodes) as TargetChoice);
		else {
			setTargetChoice("custom");
			setCustomTarget(String(preset.options.targetNodes));
		}
		setInfo(`Preset "${preset.name}" diterapkan. Klik Terapkan ulang bila gambar sudah terbuka.`);
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
				options: { ...opts, customPalette: null, paletteMerges: [], brushStrokes: [], targetNodes },
				exportOptions: exp,
			});
			setPresets(next);
			setSelectedPresetId(id);
			setPresetNameInput("");
			setAddingPreset(false);
			setInfo(`Preset "${name}" tersimpan.`);
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
			setInfo("Preset dihapus.");
		} catch (e) {
			setError(errText(e));
		}
	};

	const resetPalette = () => {
		recordHistory("Reset palet");
		setPalette(null);
		setPaletteMerges([]);
		setMergeSource(null);
		if (path && !busyRef.current) void run(path, { ...opts, customPalette: null, paletteMerges: [], targetNodes, brushStrokes: strokes });
	};

	const apply = () => {
		setMergeSource(null);
		if (path) void run(path, { ...opts, customPalette: palette, paletteMerges, targetNodes, brushStrokes: strokes });
	};

	const exportAs = async (kind: ExportKind) => {
		if (!path) return;
		if (kind === "eps" && !license.active) {
			void openPrivacyModal("license", "Ekspor EPS adalah fitur Trasea Pro (bayar sekali, 100% offline). Masukkan kunci lisensi untuk membukanya.");
			return;
		}
		setError("");
		setInfo("");
		const effExp: ExportOptions =
			opts.productionMode === "cutEmbroidery"
				? { linesAs: "shape", trapPt: 0, registrationMarks: false, stripBackground: opts.stripBackground }
				: opts.productionMode === "screenPrint"
					? { ...exp, registrationMarks: opts.registrationMarks, stripBackground: false }
					: { ...exp, registrationMarks: false, stripBackground: false };
		try {
			const target = await pickSavePath(kind, `${baseName(path)}.${kind}`);
			if (!target) return;
			await saveFile(kind, target, effExp);
			let msg = `Tersimpan: ${target}`;
			if (kind !== "ase" && withAse) {
				const ase = target.replace(/\.[^.\\/]+$/, "") + ".ase";
				await saveFile("ase", ase, effExp);
				msg += ` dan ${ase}`;
			}
			setInfo(msg);
		} catch (e) {
			setError(errText(e));
		}
	};

	const exportSeparationFilms = async () => {
		if (!path) return;
		if (!license.active) {
			void openPrivacyModal("license", "Ekspor Film Separasi Sablon adalah fitur Trasea Pro (bayar sekali, 100% offline).");
			return;
		}
		setError("");
		setInfo("");
		try {
			const dir = await pickFolder("Pilih folder untuk menyimpan film separasi");
			if (!dir) return;
			const effExp: ExportOptions = { ...exp, registrationMarks: true, stripBackground: false };
			const written = await saveSeparations(dir, baseName(path), sepPdf ? "both" : "svg", effExp);
			setInfo(`Tersimpan ${written.length} file film separasi di ${dir}`);
		} catch (e) {
			setError(errText(e));
		}
	};

	const set = (patch: Partial<Options>, label = "Ubah pengaturan") => {
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
	const svgUrl = useMemo(() => (result ? URL.createObjectURL(new Blob([result.svg], { type: "image/svg+xml" })) : ""), [result]);
	useEffect(() => () => { if (svgUrl) URL.revokeObjectURL(svgUrl); }, [svgUrl]);

	const currentPalette = (): string[] => palette ?? result?.palette ?? [];
	const removeColor = (hex: string) => {
		const base = currentPalette();
		if (base.length <= 2) return;
		recordHistory(`Hapus warna ${hex}`);
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
		recordHistory(`Gabung ${fromHex} ke ${targetHex}`);
		setPalette(base.filter(c => c !== fromHex));
		setPaletteMerges(prev => [...prev.map(([a, b]): [string, string] => [a, b === fromHex ? targetHex : b]), [fromHex, targetHex]]);
		setMergeSource(null);
		setInfo(`Warna ${fromHex} digabung ke ${targetHex}. Klik Terapkan ulang untuk memakainya.`);
	};
	const addColor = (hex: string) => {
		const base = currentPalette();
		if (base.includes(hex)) {
			setInfo(`Warna ${hex} sudah ada di palet.`);
			return;
		}
		recordHistory(`Pipet warna ${hex}`);
		setPalette([...base, hex]);
		setInfo(`Warna ${hex} ditambahkan. Klik Terapkan ulang untuk memakainya.`);
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
			recordHistory(s.kind === "detail" ? "Goresan Kuas Detail" : "Goresan Kuas Halus");
			setStrokes(list => [...list, s]);
		}
	};

	const dirty = result !== null && appliedSig !== "" && signature !== appliedSig;
	const paletteDirty = result !== null && palette !== null && JSON.stringify(palette) !== JSON.stringify(result.palette);
	const painting = tool === "detail" || tool === "smooth";
	const shown = live ? [...strokes, live] : strokes;
	const maxColors = opts.productionMode === "screenPrint" ? 8 : 16;
	const undoTip = history.past.length > 0 ? `Undo (Ctrl+Z): ${history.past[history.past.length - 1]?.label ?? ""}` : "Undo (Ctrl+Z)";
	const redoTip = history.future.length > 0 ? `Redo (Ctrl+Y): ${history.future[0]?.label ?? ""}` : "Redo (Ctrl+Y)";

	return (
		<div className={`app ${over ? "dragging" : ""}`} data-signature={signature.length}>
			<header className="top">
				<div className="brand">
					<span className="logo" aria-hidden="true">T</span> Trasea <small>1.0.0 · offline</small>
					<button
						className={`small lic-pill ${license.active ? "lic-pro" : "lic-trial"}`}
						onClick={() => void openPrivacyModal("license")}
						title="Lihat status lisensi offline dan janji privasi"
					>
						{license.active ? "Pro Aktif" : "Trial · Aktivasi"}
					</button>
				</div>
				<div className="row">
					<button aria-pressed={view === "batch"} onClick={() => setView(v => (v === "batch" ? "editor" : "batch"))} disabled={busy}>
						Batch folder{!license.active ? " (Pro)" : ""}
					</button>
					{view === "editor" && path && (
						<button className="ghost" onClick={() => void saveProject(false)} disabled={busy} title="Simpan proyek .trasea (Ctrl+S)">Simpan proyek</button>
					)}
					<button className="ghost" onClick={() => void openPrivacyModal("license")}>Privasi & Lisensi</button>
					<button onClick={() => void choose()} disabled={busy} title="Buka gambar atau proyek .trasea (Ctrl+O)">Buka gambar / proyek…</button>
				</div>
			</header>

			{view === "batch" ? (
				<BatchView
					opts={opts}
					exp={exp}
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
					<section className="stage" aria-label="Pratinjau">
						{!result && !busy && (
							<div className="drop-wrap">
								{missingSourceProject && (
									<div className="warn" style={{ marginBottom: 12 }}>
										Gambar sumber proyek tidak ditemukan di <code>{missingSourceProject}</code>. Pengaturan dan sapuan kuas tetap tersimpan.
										<div className="row" style={{ marginTop: 8 }}>
											<button onClick={() => void relinkMissingSource()}>Pilih lokasi gambar baru…</button>
										</div>
									</div>
								)}
								<div className="drop" role="button" tabIndex={0} onClick={() => void choose()} onKeyDown={e => { if (e.key === "Enter" || e.key === " ") void choose(); }}>
									<div className="drop-title">Seret gambar, proyek .trasea, atau folder ke sini</div>
									<div className="drop-sub">atau klik untuk memilih · PNG, JPG, WEBP, BMP, .trasea</div>
									<div className="drop-sub">Ilustrasi flat dan logo hasilnya paling bagus. Semua diproses 100% offline di komputer ini.</div>
								</div>
								<div className="row" style={{ justifyContent: "center", marginTop: 12 }}>
									<button onClick={() => void openSample()}>Coba contoh gambar</button>
									<button onClick={() => setView("batch")}>Proses folder (Batch)…</button>
								</div>
							</div>
						)}
						{busy && (
							<div className="progress" role="status" aria-live="polite">
								<div className="bar"><div className="fill" style={{ width: `${Math.round(fraction * 100)}%` }} /></div>
								<div>{stage}… {Math.round(fraction * 100)}%</div>
								<div className="muted">Pembersihan AI bisa memakan 10–40 detik pada gambar besar.</div>
							</div>
						)}
						{result && !busy && (
							<div className="stagebox">
								<div className="toolbar" role="toolbar" aria-label="Alat kanvas">
									<div className="group" role="group" aria-label="Mode">
										<button aria-pressed={tool === "pan"} onClick={() => setTool("pan")}>Geser</button>
										<button aria-pressed={tool === "detail"} onClick={() => setTool("detail")} className="tool-detail">Kuas Detail</button>
										<button aria-pressed={tool === "smooth"} onClick={() => setTool("smooth")} className="tool-smooth">Kuas Halus</button>
										<button aria-pressed={tool === "pipet"} onClick={() => setTool("pipet")}>Pipet Warna</button>
									</div>
									<div className="group" role="group" aria-label="Riwayat">
										<button onClick={doUndo} disabled={history.past.length === 0} title={undoTip}>Undo</button>
										<button onClick={doRedo} disabled={history.future.length === 0} title={redoTip}>Redo</button>
									</div>
									{painting && (
										<div className="group">
											<label className="brushsize" htmlFor="brush">Ukuran <b>{brushSize}</b> px</label>
											<input id="brush" type="range" min={10} max={120} value={brushSize} onChange={e => setBrushSize(Number(e.target.value))} />
											<button onClick={() => { recordHistory("Bersihkan sapuan"); setStrokes([]); }} disabled={strokes.length === 0}>Bersihkan sapuan</button>
										</div>
									)}
									<div className="group" role="group" aria-label="Overlay">
										<button aria-pressed={showHeat} onClick={() => setShowHeat(h => !h)}>Heatmap node</button>
										<button aria-pressed={checker} onClick={() => setChecker(c => !c)}>Latar kotak-kotak</button>
									</div>
								</div>
								{tool === "pipet" && <div className="hint">Klik pada gambar untuk mengambil warna dan menambahkannya ke palet.</div>}
								{painting && <div className="hint">Sapukan di atas gambar: {tool === "detail" ? "area biru memakai kurva lebih rapat (lebih detail)" : "area oranye dirampingkan (lebih sedikit node)"}. Lalu klik Terapkan ulang.</div>}
								<div className={`compare ${checker ? "checker" : ""}`} style={{ aspectRatio: `${result.width} / ${result.height}` }}>
									<img className="layer" src={result.original} alt="Gambar asli" draggable={false} style={{ clipPath: `inset(0 ${100 - split}% 0 0)` }} />
									<img className="layer vec" src={svgUrl} alt="Hasil vektor" draggable={false} style={{ clipPath: `inset(0 0 0 ${split}%)` }} />
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
									<span className="tag left">Asli</span>
									<span className="tag right">Vektor</span>
									<input className="slider" type="range" min={0} max={100} value={split} onChange={e => setSplit(Number(e.target.value))} aria-label="Geser untuk membandingkan asli dan vektor" style={{ pointerEvents: tool === "pan" ? "auto" : "none" }} />
								</div>
								{showHeat && (
									<div className="legend" aria-label="Legenda heatmap">
										<span>sedikit node</span><span className="ramp" aria-hidden="true" /><span>padat ({result.heatmap.maxCell} node/kotak)</span>
									</div>
								)}
							</div>
						)}
					</section>

					<aside className="panel" aria-label="Pengaturan">
						<div className="palette-head" style={{ marginTop: 0 }}>
							<h2>Preset Kustom</h2>
							<button className="small" onClick={() => setAddingPreset(v => !v)}>{addingPreset ? "Tutup" : "+ Simpan preset"}</button>
						</div>
						{addingPreset && (
							<div className="preset-add">
								<input
									className="num"
									type="text"
									placeholder="Nama preset (mis. Stiker Vinyl 1.5pt)"
									aria-label="Nama preset kustom"
									value={presetNameInput}
									onChange={e => setPresetNameInput(e.target.value)}
									onKeyDown={e => { if (e.key === "Enter") void createPreset(); }}
								/>
								<button className="small" onClick={() => void createPreset()} disabled={!presetNameInput.trim()}>Simpan</button>
							</div>
						)}
						{presets.length > 0 && (
							<div className="preset-row">
								<select
									aria-label="Pilih preset kustom"
									value={selectedPresetId}
									onChange={e => {
										const p = presets.find(x => x.id === e.target.value);
										if (p) applyPreset(p);
									}}
								>
									<option value="">Pilih preset tersimpan…</option>
									{presets.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
								</select>
								<button className="small" onClick={() => void removeSelectedPreset()} disabled={!selectedPresetId} aria-label="Hapus preset terpilih">Hapus</button>
							</div>
						)}

						<h2>Mode Produksi</h2>
						<div className="seg" role="group" aria-label="Mode produksi">
							<button aria-pressed={opts.productionMode === "standard"} onClick={() => switchProductionMode("standard")}>Standar</button>
							<button aria-pressed={opts.productionMode === "screenPrint"} onClick={() => switchProductionMode("screenPrint")}>Sablon</button>
							<button aria-pressed={opts.productionMode === "cutEmbroidery"} onClick={() => switchProductionMode("cutEmbroidery")}>Cutting / Bordir</button>
						</div>

						{opts.productionMode === "screenPrint" && (
							<div className="mode-box">
								<label htmlFor="minStroke">Garis minimum (pt)</label>
								<input id="minStroke" className="num" type="number" min={0} max={6} step={0.25} value={opts.minStrokePt} onChange={e => set({ minStrokePt: Math.max(0, Number(e.target.value)) }, "Ubah garis minimum")} />
								<label className="check"><input type="checkbox" checked={opts.registrationMarks} onChange={e => { const v = e.target.checked; set({ registrationMarks: v }, "Ubah tanda registrasi"); setExp(ex => ({ ...ex, registrationMarks: v })); }} /> Tanda registrasi (4 sudut)</label>
								{opts.colors > 6 && <div className="warn">Lebih dari 6 warna spot dapat menaikkan biaya film sablon.</div>}
								<div className="hint">Warna dibatasi 2–8 spot, bintik mikro diserap, garis tipis ditebalkan ke batas aman afdruk.</div>
							</div>
						)}

						{opts.productionMode === "cutEmbroidery" && (
							<div className="mode-box">
								<label className="check"><input type="checkbox" checked={opts.stripBackground} onChange={e => { const v = e.target.checked; set({ stripBackground: v }, "Ubah hapus latar"); setExp(ex => ({ ...ex, stripBackground: v })); setChecker(v); }} /> Hapus latar belakang</label>
								<label htmlFor="minStrokeCut">Garis minimum (pt)</label>
								<input id="minStrokeCut" className="num" type="number" min={0} max={6} step={0.25} value={opts.minStrokePt} onChange={e => set({ minStrokePt: Math.max(0, Number(e.target.value)) }, "Ubah garis minimum")} />
								<div className="hint">Bebas tumpang tindih (knockout), semua garis jadi bentuk isi tertutup, trap dikunci 0 pt.</div>
							</div>
						)}

						<h2>Pengaturan</h2>
						<label htmlFor="colors">Jumlah warna <b>{Math.min(opts.colors, maxColors)}</b></label>
						<input id="colors" type="range" min={2} max={maxColors} value={Math.min(opts.colors, maxColors)} onChange={e => set({ colors: Number(e.target.value) }, "Ubah jumlah warna")} />
						<label htmlFor="smooth">Kehalusan <b>{opts.smooth < 34 ? "detail" : opts.smooth > 66 ? "halus" : "sedang"}</b></label>
						<input id="smooth" type="range" min={0} max={100} value={opts.smooth} onChange={e => set({ smooth: Number(e.target.value) }, "Ubah kehalusan")} />
						<div className="hint">Kiri: lebih banyak detail dan node. Kanan: lebih sedikit node.</div>
						<label htmlFor="target">Target node (maks)</label>
						<select id="target" value={targetChoice} onChange={e => { recordHistory("Ubah target node"); setTargetChoice(e.target.value as TargetChoice); }}>
							<option value="auto">Otomatis</option>
							<option value="500">500</option>
							<option value="1000">1000</option>
							<option value="2000">2000</option>
							<option value="custom">Angka sendiri…</option>
						</select>
						{targetChoice === "custom" && <input className="num" type="number" min={1} step={50} value={customTarget} onChange={e => setCustomTarget(e.target.value)} aria-label="Target node" />}
						<div className="hint">Trasea menghaluskan hasil sampai jumlah node masuk anggaran, tanpa membersihkan gambar ulang.</div>
						<label className="check"><input type="checkbox" checked={opts.keepLines} onChange={e => set({ keepLines: e.target.checked }, "Ubah jaga garis tipis")} /> Jaga garis tipis</label>
						<label className="check"><input type="checkbox" checked={opts.detectPrimitives} onChange={e => set({ detectPrimitives: e.target.checked }, "Ubah bentuk dasar")} /> Kenali bentuk dasar</label>
						<div className="hint">Lingkaran, elips, dan persegi yang pas ditulis sebagai bentuk murni, bukan kurva perkiraan.</div>
						<label htmlFor="symmetry">Tegas simetri</label>
						<select id="symmetry" value={opts.symmetry} onChange={e => set({ symmetry: e.target.value as Options["symmetry"] }, "Ubah simetri")}>
							<option value="off">Mati</option>
							<option value="auto">Otomatis</option>
							<option value="vertical">Vertikal (kiri = kanan)</option>
							<option value="horizontal">Horizontal (atas = bawah)</option>
						</select>
						<div className="hint">Untuk logo yang seharusnya simetris. Satu sisi disalin ke sisi lain.</div>
						<label className="check"><input type="checkbox" checked={opts.aiClean} onChange={e => set({ aiClean: e.target.checked }, "Ubah pembersih AI")} /> Bersihkan dengan AI</label>
						<div className="hint">Nyalakan untuk gambar buram atau JPEG rusak. Mati: lebih cepat dan biasanya sama bagus.</div>
						{opts.aiClean && !hasSidecar && <div className="warn">Komponen AI (Real-ESRGAN) tidak ditemukan; akan dipakai filter median.</div>}
						<button className={`primary ${dirty ? "dirty" : ""}`} onClick={apply} disabled={!path || busy}>{result ? "Terapkan ulang" : "Vektorisasi"}</button>
						{dirty && <div className="pending" role="status">Perubahan belum diterapkan{palette !== null || strokes.length > 0 ? ` (${[palette !== null ? "palet" : "", strokes.length > 0 ? `${strokes.length} sapuan` : ""].filter(Boolean).join(", ")})` : ""}.</div>}

						{result && (
							<>
								<div className="palette-head">
									<h2>Palet</h2>
									<div className="row">
										<button className="small" aria-pressed={tool === "pipet"} onClick={() => setTool(t => (t === "pipet" ? "pan" : "pipet"))}>+ Pipet</button>
										<button className="small" onClick={resetPalette} disabled={palette === null && !(result.palette.length > 0 && appliedSig.includes('"customPalette":["'))}>Reset palet</button>
									</div>
								</div>
								<ul className="palette" aria-label="Palet warna">
									{currentPalette().map(c => (
										<li key={c} className="swatch-item">
											<div className="swatch">
												<span className="chip" style={{ background: c }} aria-hidden="true" />
												<code>{c}</code>
												<button className="small" aria-expanded={mergeSource === c} onClick={() => setMergeSource(s => (s === c ? null : c))} disabled={currentPalette().length <= 2} aria-label={`Gabung ${c}`}>Gabung</button>
												<button className="small" onClick={() => removeColor(c)} disabled={currentPalette().length <= 2} aria-label={`Hapus ${c}`}>Hapus</button>
											</div>
											{mergeSource === c && (
												<div className="merge-pop" role="region" aria-label={`Pilih target gabung untuk ${c}`}>
													<div className="hint">Gabung <code>{c}</code> ke warna:</div>
													<div className="merge-targets">
														{othersByDistance(c).map((t, i) => (
															<button key={t} className="small merge-opt" onClick={() => mergeColorInto(c, t)} aria-label={`Gabung ${c} ke ${t}`}>
																<span className="chip-sm" style={{ background: t }} aria-hidden="true" />
																<span>{t}{i === 0 ? " (terdekat)" : ""}</span>
															</button>
														))}
													</div>
												</div>
											)}
										</li>
									))}
								</ul>
								<div className="hint">Klik Gabung untuk memilih warna target (warna terdekat disarankan pertama). {paletteDirty ? "Palet berubah, klik Terapkan ulang." : palette !== null ? "Palet manual dipakai." : "Palet otomatis."}</div>

								<h2>Statistik</h2>
								<dl className="stats">
									<dt>Path</dt><dd>{result.paths}</dd>
									<dt>Node</dt><dd>{result.nodes.toLocaleString("id-ID")}</dd>
									<dt>Ukuran SVG</dt><dd>{fmtKb(result.bytes)}</dd>
									<dt>Bentuk dasar</dt><dd>{result.primitives}</dd>
									{result.symmetryApplied && <><dt>Simetri</dt><dd>{result.symmetryApplied === "vertical" ? "kiri = kanan" : "atas = bawah"}</dd></>}
									{result.budgetMet !== null && <><dt>Target node</dt><dd>{result.budgetMet ? "tercapai" : "tidak tercapai"}</dd><dt>Kehalusan dipakai</dt><dd>{Math.round(result.effectiveSmooth)}</dd></>}
									<dt>Selisih warna (MAE)</dt><dd>{result.mae === null ? "-" : result.mae.toFixed(2)}</dd>
									<dt>Waktu</dt><dd>{result.secs.toFixed(1)} dtk{result.reused ? " (tanpa muat/bersihkan ulang)" : ""}</dd>
									<dt>Pembersih</dt><dd>{result.usedAi ? "Real-ESRGAN" : "median"}</dd>
								</dl>
								<h2>Ekspor</h2>
								<label htmlFor="linesAs">Garis tipis sebagai</label>
								<select id="linesAs" value={opts.productionMode === "cutEmbroidery" ? "shape" : exp.linesAs} disabled={opts.productionMode === "cutEmbroidery"} onChange={e => setExp({ ...exp, linesAs: e.target.value as ExportOptions["linesAs"] })}>
									<option value="stroke">Stroke (tebal bisa diubah)</option>
									<option value="shape">Bentuk isi (outline)</option>
								</select>
								<label htmlFor="trap">Trap untuk cetak</label>
								<select id="trap" value={opts.productionMode === "cutEmbroidery" ? 0 : exp.trapPt} disabled={opts.productionMode === "cutEmbroidery"} onChange={e => setExp({ ...exp, trapPt: Number(e.target.value) })}>
									<option value={0}>Tanpa trap (0 pt)</option>
									<option value={0.25}>0,25 pt</option>
									<option value={0.5}>0,5 pt</option>
									<option value={1}>1 pt</option>
								</select>
								{opts.productionMode === "cutEmbroidery" && <div className="hint">Trap dikunci 0 pt dan garis jadi bentuk isi agar aman untuk pisau cutting / bordir.</div>}
								<label className="check"><input type="checkbox" checked={withAse} onChange={e => setWithAse(e.target.checked)} /> Sertakan palet .ase</label>
								<div className="row" style={{ marginTop: 8 }}>
									<button onClick={() => void exportAs("svg")}>Ekspor SVG</button>
									<button onClick={() => void exportAs("pdf")}>Ekspor PDF</button>
									<button onClick={() => void exportAs("eps")}>Ekspor EPS{!license.active ? " (Pro)" : ""}</button>
									<button onClick={() => void exportAs("ase")}>Palet .ase</button>
								</div>
								{opts.productionMode === "screenPrint" && (
									<div className="sep-box">
										<label className="check"><input type="checkbox" checked={sepPdf} onChange={e => setSepPdf(e.target.checked)} /> Sertakan PDF per film</label>
										<button style={{ width: "100%", marginTop: 6 }} onClick={() => void exportSeparationFilms()}>Ekspor film separasi…{!license.active ? " (Pro)" : ""}</button>
									</div>
								)}
							</>
						)}
						{result && result.textRegions.length > 0 && (
							<div className="warn" role="status" id="text-warning">
								Terdeteksi kemungkinan teks ({result.textRegions.length} area). Teks hasil vektorisasi hampir selalu kalah rapi dibanding mengetik ulang dengan font aslinya.
								<div className="row" style={{ marginTop: 8 }}>
									<button aria-pressed={showText} onClick={() => setShowText(s => !s)}>{showText ? "Sembunyikan area teks" : "Sorot area teks"}</button>
								</div>
							</div>
						)}
						{result?.notes.map(n => <div className="warn" key={n}>{n}</div>)}
						{error && <div className="error" role="alert">{error}</div>}
						{info && <div className="ok" role="status">{info}</div>}
					</aside>
				</main>
			)}

			{modalOpen !== null && (
				<div className="modal" role="dialog" aria-modal="true" aria-label="Privasi dan Lisensi" onClick={() => setModalOpen(null)}>
					<div className="modal-box" onClick={e => e.stopPropagation()}>
						<div className="row between">
							<h2>Privasi & Lisensi</h2>
							<button className="ghost" onClick={() => setModalOpen(null)}>Tutup</button>
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
