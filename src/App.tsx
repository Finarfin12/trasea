import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Licenses from "./Licenses";
import { IMAGE_EXTENSIONS, licenses, onDrop, onProgress, pickImage, pickSavePath, processImage, saveFile, sidecarAvailable, type ExportKind, type ExportOptions, type Options, type Processed } from "./api";

const DEFAULTS: Options = { colors: 10, smooth: 50, keepLines: true, aiClean: false };

const baseName = (p: string): string => (p.split(/[\\/]/).pop() ?? "gambar").replace(/\.[^.]+$/, "");
const isImage = (p: string): boolean => IMAGE_EXTENSIONS.includes((p.split(".").pop() ?? "").toLowerCase());
const fmtKb = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : "Terjadi kesalahan yang tidak dikenal.");

export default function App() {
	const [path, setPath] = useState<string | null>(null);
	const [opts, setOpts] = useState<Options>(DEFAULTS);
	const [result, setResult] = useState<Processed | null>(null);
	const [busy, setBusy] = useState(false);
	const [stage, setStage] = useState("");
	const [fraction, setFraction] = useState(0);
	const [error, setError] = useState("");
	const [info, setInfo] = useState("");
	const [over, setOver] = useState(false);
	const [split, setSplit] = useState(50);
	const [exp, setExp] = useState<ExportOptions>({ linesAs: "stroke", trapPt: 0 });
	const [withAse, setWithAse] = useState(true);
	const [checker, setChecker] = useState(false);
	const [hasSidecar, setHasSidecar] = useState(true);
	const [showLicenses, setShowLicenses] = useState<string | null>(null);
	const busyRef = useRef(false);
	const optsRef = useRef(opts);
	optsRef.current = opts;

	useEffect(() => {
		void sidecarAvailable().then(setHasSidecar);
		const subs: Array<Promise<() => void>> = [onProgress((s, f) => { setStage(s); setFraction(f); }), onDrop(paths => { const p = paths.find(isImage); if (p) void run(p); else setError("File itu bukan gambar yang didukung (PNG, JPG, WEBP, BMP)."); }, setOver)];
		return () => { subs.forEach(s => void s.then(u => u())); };
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	const run = useCallback(async (p: string, o: Options = optsRef.current) => {
		if (busyRef.current) return;
		busyRef.current = true;
		setBusy(true);
		setError("");
		setInfo("");
		setStage("Memuat gambar");
		setFraction(0);
		setPath(p);
		try {
			setResult(await processImage(p, o));
		} catch (e) {
			setError(errText(e));
		} finally {
			busyRef.current = false;
			setBusy(false);
		}
	}, []);

	const choose = async () => {
		const p = await pickImage();
		if (p) void run(p);
	};

	const exportAs = async (kind: ExportKind) => {
		if (!path) return;
		setError("");
		setInfo("");
		try {
			const target = await pickSavePath(kind, `${baseName(path)}.${kind}`);
			if (!target) return;
			await saveFile(kind, target, exp);
			let msg = `Tersimpan: ${target}`;
			if (kind !== "ase" && withAse) {
				const ase = target.replace(/\.[^.\\/]+$/, "") + ".ase";
				await saveFile("ase", ase, exp);
				msg += ` dan ${ase}`;
			}
			setInfo(msg);
		} catch (e) {
			setError(errText(e));
		}
	};

	const set = (patch: Partial<Options>) => setOpts(o => ({ ...o, ...patch }));
	const svgUrl = useMemo(() => (result ? URL.createObjectURL(new Blob([result.svg], { type: "image/svg+xml" })) : ""), [result]);
	useEffect(() => () => { if (svgUrl) URL.revokeObjectURL(svgUrl); }, [svgUrl]);

	return (
		<div className={`app ${over ? "dragging" : ""}`}>
			<header className="top">
				<div className="brand"><span className="logo" aria-hidden="true">V</span> Vektora <small>0.2.0 · offline</small></div>
				<div className="row">
					<button className="ghost" onClick={() => void licenses().then(setShowLicenses)}>Lisensi</button>
					<button onClick={() => void choose()} disabled={busy}>Buka gambar…</button>
				</div>
			</header>

			<main className="work">
				<section className="stage" aria-label="Pratinjau">
					{!result && !busy && (
						<div className="drop" role="button" tabIndex={0} onClick={() => void choose()} onKeyDown={e => { if (e.key === "Enter" || e.key === " ") void choose(); }}>
							<div className="drop-title">Seret gambar ke sini</div>
							<div className="drop-sub">atau klik untuk memilih · PNG, JPG, WEBP, BMP</div>
							<div className="drop-sub">Ilustrasi flat dan logo hasilnya paling bagus. Semua diproses di komputer ini.</div>
						</div>
					)}
					{busy && (
						<div className="progress" role="status" aria-live="polite">
							<div className="bar"><div className="fill" style={{ width: `${Math.round(fraction * 100)}%` }} /></div>
							<div>{stage}… {Math.round(fraction * 100)}%</div>
							<div className="muted">Pembersihan AI bisa memakan 10–40 detik pada gambar besar.</div>
						</div>
					)}
					{result && (
						<div className={`compare ${checker ? "checker" : ""}`} style={{ aspectRatio: `${result.width} / ${result.height}` }}>
							<img className="layer" src={result.original} alt="Gambar asli" draggable={false} />
							<img className="layer vec" src={svgUrl} alt="Hasil vektor" draggable={false} style={{ clipPath: `inset(0 0 0 ${split}%)` }} />
							<div className="divider" style={{ left: `${split}%` }} aria-hidden="true" />
							<span className="tag left">Asli</span>
							<span className="tag right">Vektor</span>
							<input className="slider" type="range" min={0} max={100} value={split} onChange={e => setSplit(Number(e.target.value))} aria-label="Geser untuk membandingkan asli dan vektor" />
						</div>
					)}
				</section>

				<aside className="panel" aria-label="Pengaturan">
					<h2>Pengaturan</h2>
					<label htmlFor="colors">Jumlah warna <b>{opts.colors}</b></label>
					<input id="colors" type="range" min={2} max={16} value={opts.colors} onChange={e => set({ colors: Number(e.target.value) })} />
					<label htmlFor="smooth">Kehalusan <b>{opts.smooth < 34 ? "detail" : opts.smooth > 66 ? "halus" : "sedang"}</b></label>
					<input id="smooth" type="range" min={0} max={100} value={opts.smooth} onChange={e => set({ smooth: Number(e.target.value) })} />
					<div className="hint">Kiri: lebih banyak detail dan node. Kanan: lebih sedikit node.</div>
					<label className="check"><input type="checkbox" checked={opts.keepLines} onChange={e => set({ keepLines: e.target.checked })} /> Jaga garis tipis</label>
					<label className="check"><input type="checkbox" checked={opts.aiClean} onChange={e => set({ aiClean: e.target.checked })} /> Bersihkan dengan AI</label>
					<div className="hint">Nyalakan untuk gambar buram atau JPEG rusak. Mati: lebih cepat dan biasanya sama bagus.</div>
					{opts.aiClean && !hasSidecar && <div className="warn">Komponen AI (Real-ESRGAN) tidak ditemukan; akan dipakai filter median.</div>}
					<button className="primary" onClick={() => path && void run(path, opts)} disabled={!path || busy}>{result ? "Terapkan ulang" : "Vektorisasi"}</button>
					<label className="check"><input type="checkbox" checked={checker} onChange={e => setChecker(e.target.checked)} /> Latar kotak-kotak</label>

					{result && (
						<>
							<h2>Statistik</h2>
							<dl className="stats">
								<dt>Path</dt><dd>{result.paths}</dd>
								<dt>Node</dt><dd>{result.nodes.toLocaleString("id-ID")}</dd>
								<dt>Ukuran SVG</dt><dd>{fmtKb(result.bytes)}</dd>
								<dt>Selisih warna (MAE)</dt><dd>{result.mae === null ? "-" : result.mae.toFixed(2)}</dd>
								<dt>Waktu</dt><dd>{result.secs.toFixed(1)} dtk</dd>
								<dt>Pembersih</dt><dd>{result.usedAi ? "Real-ESRGAN" : "median"}</dd>
							</dl>
							<div className="swatches" aria-label="Palet">{result.palette.map(c => <span key={c} title={c} style={{ background: c }} />)}</div>
							<h2>Ekspor</h2>
							<label htmlFor="linesAs">Garis tipis sebagai</label>
							<select id="linesAs" value={exp.linesAs} onChange={e => setExp({ ...exp, linesAs: e.target.value as ExportOptions["linesAs"] })}>
								<option value="stroke">Stroke (tebal bisa diubah)</option>
								<option value="shape">Bentuk isi (outline)</option>
							</select>
							<label htmlFor="trap">Trap untuk cetak</label>
							<select id="trap" value={exp.trapPt} onChange={e => setExp({ ...exp, trapPt: Number(e.target.value) })}>
								<option value={0}>Tanpa trap</option>
								<option value={0.25}>0,25 pt</option>
								<option value={0.5}>0,5 pt</option>
								<option value={1}>1 pt</option>
							</select>
							<label className="check"><input type="checkbox" checked={withAse} onChange={e => setWithAse(e.target.checked)} /> Sertakan palet .ase</label>
							<div className="row" style={{ marginTop: 8 }}>
								<button onClick={() => void exportAs("svg")}>Ekspor SVG</button>
								<button onClick={() => void exportAs("pdf")}>Ekspor PDF</button>
								<button onClick={() => void exportAs("ase")}>Palet .ase</button>
							</div>
						</>
					)}
					{result?.notes.map(n => <div className="warn" key={n}>{n}</div>)}
					{error && <div className="error" role="alert">{error}</div>}
					{info && <div className="ok" role="status">{info}</div>}
				</aside>
			</main>

			{showLicenses !== null && (
				<div className="modal" role="dialog" aria-modal="true" aria-label="Lisensi pihak ketiga" onClick={() => setShowLicenses(null)}>
					<div className="modal-box" onClick={e => e.stopPropagation()}>
						<div className="row between"><h2>Lisensi pihak ketiga</h2><button className="ghost" onClick={() => setShowLicenses(null)}>Tutup</button></div>
						<Licenses markdown={showLicenses} />
					</div>
				</div>
			)}
		</div>
	);
}
