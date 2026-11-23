import { useEffect, useMemo, useState } from "react";
import {
	cancelBatch,
	onBatchProgress,
	openFolder,
	pickFolder,
	pickImages,
	runBatch,
	scanFolder,
	type BatchItemReport,
	type BatchSummary,
	type ExportOptions,
	type LicenseStatus,
	type Options,
	type ProductionMode,
	type UserPreset,
} from "./api";

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

const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : "Terjadi kesalahan yang tidak dikenal.");
const modeLabel = (m: ProductionMode): string => (m === "screenPrint" ? "Sablon" : m === "cutEmbroidery" ? "Cutting / Bordir" : "Standar");
const parentDir = (p: string): string => p.replace(/[\\/][^\\/]+$/, "");
const joinPath = (a: string, b: string): string => `${a.replace(/[\\/]+$/, "")}\\${b}`;

export default function BatchView({ opts, exp, initialFiles, license, presets, onApplyPreset, onOpenLicenseModal, onChangeMode, onOpenInEditor, onBack }: Props) {
	const [inputDir, setInputDir] = useState<string>("");
	const [includeSubfolders, setIncludeSubfolders] = useState(false);
	const [files, setFiles] = useState<string[]>(initialFiles);
	const [outputDir, setOutputDir] = useState<string>(() => (initialFiles[0] ? joinPath(parentDir(initialFiles[0]), "trasea-output") : ""));
	const [writePdf, setWritePdf] = useState(false);
	const [writeEps, setWriteEps] = useState(false);
	const [writeAse, setWriteAse] = useState(true);
	const [writeSeparations, setWriteSeparations] = useState(opts.productionMode === "screenPrint");

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
		return () => {
			void sub.then(u => u());
		};
	}, []);

	const chooseInputFolder = async () => {
		const dir = await pickFolder("Pilih folder gambar input");
		if (!dir) return;
		setError("");
		setInputDir(dir);
		try {
			const scanned = await scanFolder(dir, includeSubfolders);
			setFiles(scanned);
			setOutputDir(joinPath(dir, "trasea-output"));
			setSummary(null);
			setLiveItems([]);
			if (scanned.length === 0) setError("Tidak ditemukan file gambar (PNG, JPG, WEBP, BMP) di folder ini.");
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
		const dir = await pickFolder("Pilih folder output batch");
		if (dir) setOutputDir(dir);
	};

	const startBatch = async () => {
		if (!license.active) {
			onOpenLicenseModal("Batch Folder adalah fitur Trasea Pro (bayar sekali, 100% offline). Masukkan kunci lisensi untuk menjalankannya.");
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
		setStage("Menyiapkan");
		try {
			const res = await runBatch({
				inputs: files,
				outputDir,
				params: { ...opts, customPalette: null, paletteMerges: [], brushStrokes: [] },
				export: exp,
				writePdf,
				writeEps,
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

	const rows = summary?.items ?? liveItems;
	const filteredRows = useMemo(() => {
		if (filter === "warn") return rows.filter(r => r.warnings.length > 0 || r.status === "failed");
		if (filter === "failed") return rows.filter(r => r.status === "failed");
		return rows;
	}, [rows, filter]);

	const warnCount = useMemo(() => rows.filter(r => r.warnings.length > 0 || r.status === "failed").length, [rows]);
	const failCount = useMemo(() => rows.filter(r => r.status === "failed").length, [rows]);
	const pct = total > 0 ? Math.round((done / total) * 100) : 0;

	return (
		<div className="batch-view">
			<div className="batch-head">
				<div className="row">
					<button className="ghost" onClick={onBack} disabled={running}>← Kembali ke Editor</button>
					<h2>Batch Folder</h2>
					{!license.active && <span className="pro-tag">Pro</span>}
				</div>
				<div className="row">
					{presets.length > 0 && (
						<select
							className="batch-preset"
							aria-label="Terapkan preset kustom ke batch"
							defaultValue=""
							disabled={running}
							onChange={e => {
								const p = presets.find(x => x.id === e.target.value);
								if (p) onApplyPreset(p);
							}}
						>
							<option value="" disabled>Terapkan preset kustom…</option>
							{presets.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
						</select>
					)}
					<div className="seg" role="group" aria-label="Mode produksi batch">
						<button aria-pressed={opts.productionMode === "standard"} disabled={running} onClick={() => onChangeMode("standard")}>Standar</button>
						<button aria-pressed={opts.productionMode === "screenPrint"} disabled={running} onClick={() => onChangeMode("screenPrint")}>Sablon</button>
						<button aria-pressed={opts.productionMode === "cutEmbroidery"} disabled={running} onClick={() => onChangeMode("cutEmbroidery")}>Cutting / Bordir</button>
					</div>
				</div>
			</div>

			{!license.active && (
				<div className="warn row between">
					<span><b>Mode Trial:</b> Batch Folder, Ekspor EPS, dan Film Separasi terbuka setelah memasukkan kunci lisensi offline Trasea Pro.</span>
					<button className="small" onClick={() => onOpenLicenseModal("Aktifkan kunci lisensi offline untuk membuka Batch Folder.")}>Aktifkan lisensi…</button>
				</div>
			)}

			<div className="batch-setup">
				<div className="batch-card">
					<div className="row between">
						<strong>Input ({files.length} gambar)</strong>
						<div className="row">
							<button className="small" onClick={() => void chooseInputFolder()} disabled={running}>Pilih folder input…</button>
							<button className="small" onClick={() => void chooseFiles()} disabled={running}>Pilih file…</button>
						</div>
					</div>
					<div className="batch-path">{inputDir || (files.length > 0 ? `${files.length} file dipilih` : "Belum ada folder/file dipilih")}</div>
					<label className="check"><input type="checkbox" checked={includeSubfolders} disabled={running || !inputDir} onChange={e => void toggleSubfolders(e.target.checked)} /> Sertakan sub-folder (1 tingkat)</label>
				</div>

				<div className="batch-card">
					<div className="row between">
						<strong>Folder Output</strong>
						<button className="small" onClick={() => void chooseOutputFolder()} disabled={running}>Ubah folder…</button>
					</div>
					<div className="batch-path">{outputDir || "Pilih folder tujuan"}</div>
					<div className="row" style={{ marginTop: 8 }}>
						<label className="check inline"><input type="checkbox" checked disabled /> SVG</label>
						<label className="check inline"><input type="checkbox" checked={writePdf} disabled={running} onChange={e => setWritePdf(e.target.checked)} /> PDF</label>
						<label className="check inline"><input type="checkbox" checked={writeEps} disabled={running} onChange={e => setWriteEps(e.target.checked)} /> EPS</label>
						<label className="check inline"><input type="checkbox" checked={writeAse} disabled={running} onChange={e => setWriteAse(e.target.checked)} /> ASE</label>
						{opts.productionMode === "screenPrint" && (
							<label className="check inline"><input type="checkbox" checked={writeSeparations} disabled={running} onChange={e => setWriteSeparations(e.target.checked)} /> Film separasi (#000000)</label>
						)}
					</div>
				</div>
			</div>

			<div className="batch-params-bar">
				<span>Mode: <b>{modeLabel(opts.productionMode)}</b></span>
				<span>Warna: <b>{opts.colors}</b></span>
				<span>Kehalusan: <b>{opts.smooth}</b></span>
				{opts.productionMode === "screenPrint" && <span>Garis min: <b>{opts.minStrokePt} pt</b> · Registrasi: <b>{opts.registrationMarks ? "ya" : "tidak"}</b></span>}
				{opts.productionMode === "cutEmbroidery" && <span>Hapus latar: <b>{opts.stripBackground ? "ya" : "tidak"}</b> · Trap: <b>0 pt</b></span>}
				<span>Pembersih: <b>{opts.aiClean ? "Real-ESRGAN (berurutan)" : "Median (paralel)"}</b></span>
			</div>

			<div className="batch-actions">
				<div className="row">
					<button className="primary batch-run" onClick={() => void startBatch()} disabled={running || files.length === 0 || !outputDir}>
						{running ? `Memproses (${done}/${total})…` : !license.active ? `Mulai batch (Pro)` : `Mulai batch (${files.length} file)`}
					</button>
					{running && (
						<button className="danger" onClick={() => void stopBatch()} disabled={cancelling}>
							{cancelling ? "Membatalkan…" : "Batal"}
						</button>
					)}
					{summary && outputDir && (
						<button onClick={() => void openFolder(outputDir).catch(e => setError(errText(e)))}>Buka folder output</button>
					)}
				</div>
				{(running || summary) && (
					<div className="batch-prog">
						<div className="bar"><div className="fill" style={{ width: `${summary ? 100 : pct}%` }} /></div>
						<div className="muted">
							{running
								? `${done} / ${total} — ${currentFile || "Menyiapkan"} (${stage})`
								: summary
									? `Selesai dalam ${summary.totalSecs.toFixed(1)} dtk · Sukses: ${summary.succeeded}, Gagal: ${summary.failed}, Dibatalkan: ${summary.cancelled} · Laporan tersimpan (laporan-trasea.csv & laporan-trasea.md)`
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
							<button className="small" aria-pressed={filter === "all"} onClick={() => setFilter("all")}>Semua ({rows.length})</button>
							<button className="small" aria-pressed={filter === "warn"} onClick={() => setFilter("warn")}>Ada Peringatan ({warnCount})</button>
							<button className="small" aria-pressed={filter === "failed"} onClick={() => setFilter("failed")}>Gagal ({failCount})</button>
						</div>
						<span className="hint">Klik satu baris untuk membuka gambar di editor tunggal.</span>
					</div>
					<div className="batch-table-wrap">
						<table className="batch-table">
							<thead>
								<tr>
									<th>Nama file</th>
									<th>Status</th>
									<th className="num-col">Warna</th>
									<th className="num-col">Path</th>
									<th className="num-col">Node</th>
									<th className="num-col">MAE</th>
									<th>Peringatan</th>
									<th className="num-col">Waktu</th>
								</tr>
							</thead>
							<tbody>
								{filteredRows.map((it, idx) => (
									<tr key={`${it.inputPath}-${idx}`} className={`row-${it.status}`} onClick={() => { if (!running) onOpenInEditor(it.inputPath); }} title="Klik untuk poles di editor tunggal">
										<td>{it.fileName}</td>
										<td><span className={`badge badge-${it.status}`}>{it.status === "ok" ? "Sukses" : it.status === "failed" ? "Gagal" : "Dibatalkan"}</span></td>
										<td className="num-col">{it.colors || "-"}</td>
										<td className="num-col">{it.paths || "-"}</td>
										<td className="num-col">{it.nodes ? it.nodes.toLocaleString("id-ID") : "-"}</td>
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
		</div>
	);
}
