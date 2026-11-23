import { useEffect, useMemo, useState } from "react";
import {
	cancelBatch,
	onBatchProgress,
	onWatchEvent,
	onWatchStopped,
	openFolder,
	pickFolder,
	pickImages,
	runBatch,
	scanFolder,
	startWatchFolder,
	stopWatchFolder,
	type BatchItemReport,
	type BatchSummary,
	type ColorSpace,
	type DxfUnit,
	type ExportOptions,
	type LicenseStatus,
	type Options,
	type ProductionMode,
	type UserPreset,
	type WatchEvent,
} from "./api";
import { useI18n } from "./i18n";

interface Props {
	opts: Options;
	exp: ExportOptions;
	initialFiles: string[];
	license: LicenseStatus;
	presets: UserPreset[];
	onApplyPreset: (preset: UserPreset) => void;
	onOpenLicenseModal: (hint?: string) => void;
	onChangeMode: (mode: ProductionMode) => void;
	onOpenInEditor: (filePath: string) => void;
	onBack: () => void;
}

type Filter = "all" | "warn" | "failed";
type SubTab = "batch" | "watch";

const parentDir = (p: string): string => p.replace(/[\\/][^\\/]+$/, "");
const joinPath = (a: string, b: string): string => `${a.replace(/[\\/]+$/, "")}\\${b}`;

export default function BatchView({ opts, exp, initialFiles, license, presets, onApplyPreset, onOpenLicenseModal, onChangeMode, onOpenInEditor, onBack }: Props) {
	const { t, lang } = useI18n();
	const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : t.unknownError);
	const modeLabel = (m: ProductionMode): string =>
		m === "screenPrint" ? t.modeScreenPrint : m === "cutEmbroidery" ? t.modeCutEmbroidery : m === "shading" ? t.modeShading : t.modeStandard;

	const [subTab, setSubTab] = useState<SubTab>("batch");

	// Shared format/export toggles
	const [writePdf, setWritePdf] = useState(false);
	const [writeEps, setWriteEps] = useState(false);
	const [writeDxf, setWriteDxf] = useState(false);
	const [writeAse, setWriteAse] = useState(true);
	const [writeSeparations, setWriteSeparations] = useState(opts.productionMode === "screenPrint");
	const [colorSpace, setColorSpace] = useState<ColorSpace>(exp.colorSpace);
	const [dxfUnit, setDxfUnit] = useState<DxfUnit>(exp.dxfUnit);
	const [dxfWidthStr, setDxfWidthStr] = useState<string>(exp.dxfTargetWidthMm ? String(exp.dxfTargetWidthMm) : "");

	// Batch once state
	const [inputDir, setInputDir] = useState<string>("");
	const [includeSubfolders, setIncludeSubfolders] = useState(false);
	const [files, setFiles] = useState<string[]>(initialFiles);
	const [outputDir, setOutputDir] = useState<string>(() => (initialFiles[0] ? joinPath(parentDir(initialFiles[0]), "trasea-output") : ""));
	const [running, setRunning] = useState(false);
	const [cancelling, setCancelling] = useState(false);
	const [done, setDone] = useState(0);
	const [total, setTotal] = useState(0);
	const [currentFile, setCurrentFile] = useState("");
	const [stage, setStage] = useState("");
	const [liveItems, setLiveItems] = useState<BatchItemReport[]>([]);
	const [summary, setSummary] = useState<BatchSummary | null>(null);
	const [filter, setFilter] = useState<Filter>("all");
	const [error, setError] = useState("");

	// Watch folder state
	const [watchInDir, setWatchInDir] = useState<string>("");
	const [watchOutDir, setWatchOutDir] = useState<string>("");
	const [watchSubfolders, setWatchSubfolders] = useState<boolean>(false);
	const [watching, setWatching] = useState<boolean>(false);
	const [watchStopping, setWatchStopping] = useState<boolean>(false);
	const [watchEvents, setWatchEvents] = useState<WatchEvent[]>([]);
	const [watchError, setWatchError] = useState<string>("");

	useEffect(() => {
		if (initialFiles.length > 0) {
			setFiles(initialFiles);
			setInputDir("");
			const f0 = initialFiles[0];
			if (!outputDir && f0) setOutputDir(joinPath(parentDir(f0), "trasea-output"));
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [initialFiles]);

	useEffect(() => {
		setWriteSeparations(opts.productionMode === "screenPrint");
	}, [opts.productionMode]);

	useEffect(() => {
		const sub = onBatchProgress(prog => {
			setDone(prog.done);
			setTotal(prog.total);
			setCurrentFile(prog.currentFile);
			setStage(prog.stage);
			if (prog.itemReport) {
				setLiveItems(prev => [...prev, prog.itemReport as BatchItemReport]);
			}
		});
		const subW = onWatchEvent(ev => {
			setWatchEvents(prev => [ev, ...prev]);
		});
		const subWS = onWatchStopped(ev => {
			setWatching(false);
			setWatchStopping(false);
			if (ev.error) setWatchError(ev.error);
		});
		return () => {
			void sub.then(u => u());
			void subW.then(u => u());
			void subWS.then(u => u());
		};
	}, []);

	const parsedDxfWidth = useMemo(() => {
		const v = Number.parseFloat(dxfWidthStr);
		return Number.isFinite(v) && v > 0 ? v : null;
	}, [dxfWidthStr]);

	const effExport: ExportOptions = useMemo(
		() => ({
			...exp,
			colorSpace,
			dxfUnit,
			dxfTargetWidthMm: dxfUnit === "mm" ? parsedDxfWidth : null,
		}),
		[exp, colorSpace, dxfUnit, parsedDxfWidth],
	);

	const busyAny = running || watching;

	const chooseInputFolder = async () => {
		const dir = await pickFolder(t.chooseInputFolder);
		if (!dir) return;
		setError("");
		setInputDir(dir);
		try {
			const scanned = await scanFolder(dir, includeSubfolders);
			setFiles(scanned);
			setOutputDir(joinPath(dir, "trasea-output"));
			setSummary(null);
			setLiveItems([]);
			if (scanned.length === 0) setError(t.noImagesInFolder);
		} catch (e) {
			setError(errText(e));
		}
	};

	const toggleSubfolders = async (checked: boolean) => {
		setIncludeSubfolders(checked);
		if (!inputDir) return;
		setError("");
		try {
			const scanned = await scanFolder(inputDir, checked);
			setFiles(scanned);
		} catch (e) {
			setError(errText(e));
		}
	};

	const chooseFiles = async () => {
		const picked = await pickImages();
		if (picked.length === 0) return;
		setError("");
		setInputDir("");
		setFiles(picked);
		const p0 = picked[0];
		if (!outputDir && p0) setOutputDir(joinPath(parentDir(p0), "trasea-output"));
		setSummary(null);
		setLiveItems([]);
	};

	const chooseOutputFolder = async () => {
		const dir = await pickFolder(t.outputFolderLabel);
		if (dir) setOutputDir(dir);
	};

	const startBatch = async () => {
		if (!license.active) {
			onOpenLicenseModal(t.batchProPrompt);
			return;
		}
		if (files.length === 0 || !outputDir || running) return;
		setRunning(true);
		setCancelling(false);
		setError("");
		setSummary(null);
		setLiveItems([]);
		setDone(0);
		setTotal(files.length);
		setCurrentFile("");
		setStage(t.preparing);
		try {
			const res = await runBatch({
				inputs: files,
				outputDir,
				params: { ...opts, customPalette: null, paletteMerges: [], brushStrokes: [] },
				export: effExport,
				writePdf,
				writeEps,
				writeDxf,
				writeAse,
				writeSeparations: opts.productionMode === "screenPrint" && writeSeparations,
			});
			setSummary(res);
			setLiveItems(res.items);
		} catch (e) {
			setError(errText(e));
		} finally {
			setRunning(false);
			setCancelling(false);
		}
	};

	const stopBatch = async () => {
		setCancelling(true);
		try {
			await cancelBatch();
		} catch (e) {
			setError(errText(e));
		}
	};

	// Watch Folder actions (OPEN without Pro key per QT-1=B)
	const chooseWatchIn = async () => {
		const dir = await pickFolder(t.watchChooseInput);
		if (!dir) return;
		setWatchError("");
		setWatchInDir(dir);
		if (!watchOutDir) setWatchOutDir(joinPath(dir, "trasea-output"));
	};

	const chooseWatchOut = async () => {
		const dir = await pickFolder(t.outputFolderLabel);
		if (dir) setWatchOutDir(dir);
	};

	const doStartWatch = async () => {
		if (!watchInDir || !watchOutDir || watching) return;
		setWatchError("");
		try {
			await startWatchFolder({
				inDir: watchInDir,
				outDir: watchOutDir,
				includeSubfolders: watchSubfolders,
				params: { ...opts, customPalette: null, paletteMerges: [], brushStrokes: [] },
				export: effExport,
				writePdf,
				writeEps,
				writeDxf,
				writeAse,
				writeSeparations: opts.productionMode === "screenPrint" && writeSeparations,
				pollMs: 1000,
				debounceMs: 500,
			});
			setWatching(true);
			setWatchStopping(false);
		} catch (e) {
			setWatchError(errText(e));
		}
	};

	const doStopWatch = async () => {
		setWatchStopping(true);
		try {
			await stopWatchFolder();
		} catch (e) {
			setWatchError(errText(e));
			setWatchStopping(false);
		}
	};

	const rows = summary?.items ?? liveItems;
	const filteredRows = useMemo(() => {
		if (filter === "warn") return rows.filter(r => r.warnings.length > 0 || r.status === "failed");
		if (filter === "failed") return rows.filter(r => r.status === "failed");
		return rows;
	}, [rows, filter]);

	const warnCount = useMemo(() => rows.filter(r => r.warnings.length > 0 || r.status === "failed").length, [rows]);
	const failCount = useMemo(() => rows.filter(r => r.status === "failed").length, [rows]);
	const pct = total > 0 ? Math.round((done / total) * 100) : 0;
	const locale = lang === "en" ? "en-US" : "id-ID";

	return (
		<div className="batch-view">
			<div className="batch-head">
				<div className="row">
					<button className="ghost" onClick={onBack} disabled={busyAny}>{t.backToEditor}</button>
					<h2>{t.batchTitle}</h2>
					<div className="seg seg2" role="tablist" aria-label={t.batchTitle}>
						<button role="tab" aria-selected={subTab === "batch"} aria-pressed={subTab === "batch"} disabled={busyAny} onClick={() => setSubTab("batch")}>{t.tabBatchOnce}</button>
						<button role="tab" aria-selected={subTab === "watch"} aria-pressed={subTab === "watch"} disabled={busyAny} onClick={() => setSubTab("watch")}>{t.tabWatchFolder}</button>
					</div>
				</div>
				<div className="row">
					{presets.length > 0 && (
						<select
							className="batch-preset"
							aria-label={t.applyPresetToBatch}
							defaultValue=""
							disabled={busyAny}
							onChange={e => {
								const p = presets.find(x => x.id === e.target.value);
								if (p) onApplyPreset(p);
							}}
						>
							<option value="" disabled>{t.applyPresetToBatch}</option>
							{presets.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
						</select>
					)}
					<div className="seg seg4" role="group" aria-label={t.productionModeAria}>
						<button aria-pressed={opts.productionMode === "standard"} disabled={busyAny} onClick={() => onChangeMode("standard")}>{t.modeStandard}</button>
						<button aria-pressed={opts.productionMode === "screenPrint"} disabled={busyAny} onClick={() => onChangeMode("screenPrint")}>{t.modeScreenPrint}</button>
						<button aria-pressed={opts.productionMode === "cutEmbroidery"} disabled={busyAny} onClick={() => onChangeMode("cutEmbroidery")}>{t.modeCutEmbroidery}</button>
						<button aria-pressed={opts.productionMode === "shading"} disabled={busyAny} onClick={() => onChangeMode("shading")}>{t.modeShading}</button>
					</div>
				</div>
			</div>

			{!license.active && subTab === "batch" && (
				<div className="warn row between">
					<span>{t.trialBatchBanner}</span>
					<button className="small" onClick={() => onOpenLicenseModal(t.batchProPrompt)}>{t.activateLicenseBtn}</button>
				</div>
			)}

			{subTab === "batch" ? (
				<>
					<div className="batch-setup">
						<div className="batch-card">
							<div className="row between">
								<strong>{t.inputFilesLabel} ({files.length} {t.imagesUnit})</strong>
								<div className="row">
									<button className="small" onClick={() => void chooseInputFolder()} disabled={running}>{t.chooseInputFolder}</button>
									<button className="small" onClick={() => void chooseFiles()} disabled={running}>{t.chooseFiles}</button>
								</div>
							</div>
							<div className="batch-path">{inputDir || (files.length > 0 ? `${files.length} ${t.filesSelectedSuffix}` : t.noInputSelected)}</div>
							<label className="check"><input type="checkbox" checked={includeSubfolders} disabled={running || !inputDir} onChange={e => void toggleSubfolders(e.target.checked)} /> {t.includeSubfolders}</label>
						</div>

						<div className="batch-card">
							<div className="row between">
								<strong>{t.outputFolderLabel}</strong>
								<button className="small" onClick={() => void chooseOutputFolder()} disabled={running}>{t.changeFolder}</button>
							</div>
							<div className="batch-path">{outputDir || t.chooseOutputPlaceholder}</div>
							<div className="row" style={{ marginTop: 8 }}>
								<label className="check inline"><input type="checkbox" checked disabled /> SVG</label>
								<label className="check inline"><input type="checkbox" checked={writePdf} disabled={running} onChange={e => setWritePdf(e.target.checked)} /> PDF</label>
								<label className="check inline"><input type="checkbox" checked={writeEps} disabled={running} onChange={e => setWriteEps(e.target.checked)} /> EPS</label>
								<label className="check inline"><input type="checkbox" checked={writeDxf} disabled={running} onChange={e => setWriteDxf(e.target.checked)} /> DXF</label>
								<label className="check inline"><input type="checkbox" checked={writeAse} disabled={running} onChange={e => setWriteAse(e.target.checked)} /> ASE</label>
								{opts.productionMode === "screenPrint" && (
									<label className="check inline"><input type="checkbox" checked={writeSeparations} disabled={running} onChange={e => setWriteSeparations(e.target.checked)} /> {t.sepFilmsShort}</label>
								)}
							</div>
							<div className="row" style={{ marginTop: 8 }}>
								<span className="muted">{t.colorSpaceLabel}:</span>
								<div className="seg seg2 compact-seg" role="group" aria-label={t.colorSpaceLabel}>
									<button aria-pressed={colorSpace === "rgb"} disabled={running} onClick={() => setColorSpace("rgb")}>RGB</button>
									<button aria-pressed={colorSpace === "cmyk"} disabled={running} onClick={() => setColorSpace("cmyk")}>CMYK</button>
								</div>
								{writeDxf && (
									<>
										<select className="batch-preset" value={dxfUnit} disabled={running} onChange={e => setDxfUnit(e.target.value as DxfUnit)} aria-label={t.dxfUnitLabel}>
											<option value="mm">DXF: mm</option>
											<option value="px">DXF: px</option>
										</select>
										{dxfUnit === "mm" && (
											<input
												className="batch-preset"
												style={{ width: 110 }}
												type="number"
												min={1}
												step={1}
												disabled={running}
												placeholder={t.dxfTargetWidthPlaceholder}
												value={dxfWidthStr}
												onChange={e => setDxfWidthStr(e.target.value)}
												aria-label={t.dxfTargetWidthLabel}
											/>
										)}
									</>
								)}
							</div>
						</div>
					</div>

					<div className="batch-params-bar">
						<span>{t.batchMode} <b>{modeLabel(opts.productionMode)}</b></span>
						<span>{t.batchColors} <b>{opts.colors}</b></span>
						<span>{t.batchSmooth} <b>{opts.smooth}</b></span>
						<span>{t.batchColorSpace} <b>{colorSpace.toUpperCase()}</b></span>
						{opts.productionMode === "screenPrint" && <span>{t.batchMinStroke} <b>{opts.minStrokePt} pt</b> · {t.batchReg} <b>{opts.registrationMarks ? t.yes : t.no}</b></span>}
						{opts.productionMode === "cutEmbroidery" && <span>{t.batchStripBg} <b>{opts.stripBackground ? t.yes : t.no}</b> · Trap: <b>0 pt</b></span>}
						<span>{t.batchCleaner} <b>{opts.aiClean ? t.cleanerAiSeq : t.cleanerMedPar}</b></span>
					</div>

					<div className="batch-actions">
						<div className="row">
							<button className="primary batch-run" onClick={() => void startBatch()} disabled={running || files.length === 0 || !outputDir}>
								{running ? `${t.processingLabel} (${done}/${total})…` : !license.active ? t.startBatchPro : `${t.startBatch} (${files.length})`}
							</button>
							{running && (
								<button className="danger" onClick={() => void stopBatch()} disabled={cancelling}>
									{cancelling ? t.cancelling : t.cancel}
								</button>
							)}
							{summary && outputDir && (
								<button onClick={() => void openFolder(outputDir).catch(e => setError(errText(e)))}>{t.openOutputFolder}</button>
							)}
						</div>
						{(running || summary) && (
							<div className="batch-prog">
								<div className="bar"><div className="fill" style={{ width: `${summary ? 100 : pct}%` }} /></div>
								<div className="muted">
									{running
										? `${done} / ${total} — ${currentFile || t.preparing} (${stage})`
										: summary
											? `${t.doneIn} ${summary.totalSecs.toFixed(1)} ${t.secUnit} · ${t.succeeded}: ${summary.succeeded}, ${t.failed}: ${summary.failed}, ${t.cancelled}: ${summary.cancelled} · ${t.reportSavedNote}`
											: ""}
								</div>
							</div>
						)}
					</div>

					{error && <div className="error" role="alert">{error}</div>}

					{rows.length > 0 && (
						<div className="batch-report">
							<div className="row between" style={{ marginBottom: 8 }}>
								<div className="row">
									<button className="small" aria-pressed={filter === "all"} onClick={() => setFilter("all")}>{t.filterAll} ({rows.length})</button>
									<button className="small" aria-pressed={filter === "warn"} onClick={() => setFilter("warn")}>{t.filterWarn} ({warnCount})</button>
									<button className="small" aria-pressed={filter === "failed"} onClick={() => setFilter("failed")}>{t.filterFailed} ({failCount})</button>
								</div>
								<span className="hint">{t.clickRowHint}</span>
							</div>
							<div className="batch-table-wrap">
								<table className="batch-table">
									<thead>
										<tr>
											<th>{t.colFileName}</th>
											<th>{t.colStatus}</th>
											<th className="num-col">{t.colColors}</th>
											<th className="num-col">{t.colPaths}</th>
											<th className="num-col">{t.colNodes}</th>
											<th className="num-col">{t.colMae}</th>
											<th>{t.colWarnings}</th>
											<th className="num-col">{t.colTime}</th>
										</tr>
									</thead>
									<tbody>
										{filteredRows.map((it, idx) => (
											<tr key={`${it.inputPath}-${idx}`} className={`row-${it.status}`} onClick={() => { if (!running) onOpenInEditor(it.inputPath); }} title={t.clickRowTitle}>
												<td>{it.fileName}</td>
												<td><span className={`badge badge-${it.status}`}>{it.status === "ok" ? t.statusOk : it.status === "failed" ? t.statusFailed : t.statusCancelled}</span></td>
												<td className="num-col">{it.colors || "-"}</td>
												<td className="num-col">{it.paths || "-"}</td>
												<td className="num-col">{it.nodes ? it.nodes.toLocaleString(locale) : "-"}</td>
												<td className="num-col">{it.mae !== null ? it.mae.toFixed(2) : "-"}</td>
												<td>{it.error ? `Error: ${it.error}` : it.warnings.length > 0 ? it.warnings.join("; ") : "-"}</td>
												<td className="num-col">{it.secs > 0 ? `${it.secs.toFixed(1)}s` : "-"}</td>
											</tr>
										))}
									</tbody>
								</table>
							</div>
						</div>
					)}
				</>
			) : (
				<>
					<div className="hint">{t.watchHint}</div>
					<div className="batch-setup">
						<div className="batch-card">
							<div className="row between">
								<strong>{t.watchInputLabel}</strong>
								<button className="small" onClick={() => void chooseWatchIn()} disabled={watching}>{t.watchChooseInput}</button>
							</div>
							<div className="batch-path">{watchInDir || t.noInputSelected}</div>
							<label className="check"><input type="checkbox" checked={watchSubfolders} disabled={watching} onChange={e => setWatchSubfolders(e.target.checked)} /> {t.includeSubfolders}</label>
						</div>

						<div className="batch-card">
							<div className="row between">
								<strong>{t.outputFolderLabel}</strong>
								<button className="small" onClick={() => void chooseWatchOut()} disabled={watching}>{t.changeFolder}</button>
							</div>
							<div className="batch-path">{watchOutDir || t.chooseOutputPlaceholder}</div>
							<div className="row" style={{ marginTop: 8 }}>
								<label className="check inline"><input type="checkbox" checked disabled /> SVG</label>
								<label className="check inline"><input type="checkbox" checked={writePdf} disabled={watching} onChange={e => setWritePdf(e.target.checked)} /> PDF</label>
								<label className="check inline"><input type="checkbox" checked={writeEps} disabled={watching} onChange={e => setWriteEps(e.target.checked)} /> EPS</label>
								<label className="check inline"><input type="checkbox" checked={writeDxf} disabled={watching} onChange={e => setWriteDxf(e.target.checked)} /> DXF</label>
								<label className="check inline"><input type="checkbox" checked={writeAse} disabled={watching} onChange={e => setWriteAse(e.target.checked)} /> ASE</label>
								{opts.productionMode === "screenPrint" && (
									<label className="check inline"><input type="checkbox" checked={writeSeparations} disabled={watching} onChange={e => setWriteSeparations(e.target.checked)} /> {t.sepFilmsShort}</label>
								)}
							</div>
							<div className="row" style={{ marginTop: 8 }}>
								<span className="muted">{t.colorSpaceLabel}:</span>
								<div className="seg seg2 compact-seg" role="group" aria-label={t.colorSpaceLabel}>
									<button aria-pressed={colorSpace === "rgb"} disabled={watching} onClick={() => setColorSpace("rgb")}>RGB</button>
									<button aria-pressed={colorSpace === "cmyk"} disabled={watching} onClick={() => setColorSpace("cmyk")}>CMYK</button>
								</div>
								{writeDxf && (
									<select className="batch-preset" value={dxfUnit} disabled={watching} onChange={e => setDxfUnit(e.target.value as DxfUnit)} aria-label={t.dxfUnitLabel}>
										<option value="mm">DXF: mm</option>
										<option value="px">DXF: px</option>
									</select>
								)}
							</div>
						</div>
					</div>

					<div className="batch-actions">
						<div className="row between">
							<div className="row">
								{!watching ? (
									<button className="primary batch-run" onClick={() => void doStartWatch()} disabled={!watchInDir || !watchOutDir}>
										{t.watchStart}
									</button>
								) : (
									<button className="danger batch-run" onClick={() => void doStopWatch()} disabled={watchStopping}>
										{watchStopping ? t.cancelling : t.watchStop}
									</button>
								)}
								<span className={`badge ${watching ? "badge-ok" : "badge-cancelled"}`}>
									{watching ? `● ${t.watchActiveBadge}` : t.watchIdleBadge}
								</span>
								<span className="muted">{t.watchProcessedCount}: <b>{watchEvents.length}</b></span>
							</div>
							<div className="row">
								{watchEvents.length > 0 && (
									<button className="small" onClick={() => setWatchEvents([])}>{t.watchClearLog}</button>
								)}
								{watchOutDir && (
									<button className="small" onClick={() => void openFolder(watchOutDir).catch(e => setWatchError(errText(e)))}>{t.openOutputFolder}</button>
								)}
							</div>
						</div>
					</div>

					{watchError && <div className="error" role="alert">{watchError}</div>}

					<div className="batch-report">
						{watchEvents.length === 0 ? (
							<div className="batch-card muted" style={{ textAlign: "center", padding: "28px 16px" }}>
								{t.watchEmptyLog}
							</div>
						) : (
							<div className="batch-table-wrap">
								<table className="batch-table">
									<thead>
										<tr>
											<th>{t.colFileName}</th>
											<th>{t.colStatus}</th>
											<th className="num-col">{t.colColors}</th>
											<th className="num-col">{t.colPaths}</th>
											<th className="num-col">{t.colNodes}</th>
											<th className="num-col">{t.colMae}</th>
											<th>{t.colWarnings}</th>
											<th className="num-col">{t.colTime}</th>
										</tr>
									</thead>
									<tbody>
										{watchEvents.map((ev, idx) => (
											<tr key={`${ev.file}-${idx}`} className={`row-${ev.status}`} onClick={() => { if (!watching) onOpenInEditor(ev.file); }} title={t.clickRowTitle}>
												<td>{ev.fileName}</td>
												<td><span className={`badge badge-${ev.status}`}>{ev.status === "ok" ? t.statusOk : ev.status === "failed" ? t.statusFailed : t.statusCancelled}</span></td>
												<td className="num-col">{ev.colors || "-"}</td>
												<td className="num-col">{ev.paths || "-"}</td>
												<td className="num-col">{ev.nodes ? ev.nodes.toLocaleString(locale) : "-"}</td>
												<td className="num-col">{ev.mae !== null ? ev.mae.toFixed(2) : "-"}</td>
												<td>{ev.error ? `Error: ${ev.error}` : "-"}</td>
												<td className="num-col">{(ev.elapsedMs / 1000).toFixed(2)}s</td>
											</tr>
										))}
									</tbody>
								</table>
							</div>
						)}
					</div>
				</>
			)}
		</div>
	);
}
