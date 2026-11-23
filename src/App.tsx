import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import Licenses from "./Licenses";
import { IMAGE_EXTENSIONS, licenses, onDrop, onProgress, pickColor, pickImage, pickSavePath, processImage, saveFile, sidecarAvailable, type BrushStroke, type ExportKind, type ExportOptions, type Options, type Processed } from "./api";

const DEFAULTS: Options = { colors: 10, smooth: 50, keepLines: true, aiClean: false, detectPrimitives: true, symmetry: "off", detectText: true, customPalette: null, targetNodes: null, brushStrokes: [] };

type Tool = "pan" | "detail" | "smooth" | "pipet";
type TargetChoice = "auto" | "500" | "1000" | "2000" | "custom";

const baseName = (p: string): string => (p.split(/[\\/]/).pop() ?? "gambar").replace(/\.[^.]+$/, "");
const isImage = (p: string): boolean => IMAGE_EXTENSIONS.includes((p.split(".").pop() ?? "").toLowerCase());
const fmtKb = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : "Terjadi kesalahan yang tidak dikenal.");
const rgb = (hex: string): [number, number, number] => [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];
const heatColor = (r: number): string => `hsl(${Math.round(120 * (1 - Math.min(1, Math.max(0, r))))}, 90%, 50%)`;

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
	/** Show boxes around suspected text areas in the preview (off by default; never exported). */
	const [showText, setShowText] = useState(false);
	/** Node heatmap over the preview (off by default; never exported). */
	const [showHeat, setShowHeat] = useState(false);
	const [tool, setTool] = useState<Tool>("pan");
	const [brushSize, setBrushSize] = useState(40);
	const [strokes, setStrokes] = useState<BrushStroke[]>([]);
	const [live, setLive] = useState<BrushStroke | null>(null);
	/** The designer's palette; null = automatic. Changes wait for "Terapkan ulang". */
	const [palette, setPalette] = useState<string[] | null>(null);
	const [targetChoice, setTargetChoice] = useState<TargetChoice>("auto");
	const [customTarget, setCustomTarget] = useState("800");
	const [hasSidecar, setHasSidecar] = useState(true);
	const [showLicenses, setShowLicenses] = useState<string | null>(null);
	/** What the last conversion was made with, to show that there are changes that are not applied yet. */
	const [appliedSig, setAppliedSig] = useState("");
	const busyRef = useRef(false);
	const optsRef = useRef(opts);
	optsRef.current = opts;
	const overlayRef = useRef<SVGSVGElement | null>(null);
	const liveRef = useRef<BrushStroke | null>(null);

	const targetNodes = useMemo((): number | null => {
		if (targetChoice === "auto") return null;
		const n = targetChoice === "custom" ? Number.parseInt(customTarget, 10) : Number.parseInt(targetChoice, 10);
		return Number.isFinite(n) && n > 0 ? n : null;
	}, [targetChoice, customTarget]);

	/** All the settings that change the result, as one string. */
	const signature = useMemo(() => JSON.stringify({ ...opts, customPalette: palette, targetNodes, brushStrokes: strokes }), [opts, palette, targetNodes, strokes]);

	useEffect(() => {
		void sidecarAvailable().then(setHasSidecar);
		const subs: Array<Promise<() => void>> = [onProgress((s, f) => { setStage(s); setFraction(f); }), onDrop(paths => { const p = paths.find(isImage); if (p) openPicture(p); else setError("File itu bukan gambar yang didukung (PNG, JPG, WEBP, BMP)."); }, setOver)];
		return () => { subs.forEach(s => void s.then(u => u())); };
		// eslint-disable-next-line react-hooks/exhaustive-deps
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

	/** A new picture: brush strokes and palette belong to the old one. */
	const openPicture = (p: string) => {
		setStrokes([]);
		setLive(null);
		setPalette(null);
		setTool("pan");
		void run(p, { ...optsRef.current, customPalette: null, brushStrokes: [] });
	};

	/** Back to the automatic palette: the list would show the old manual colours until the next run, so run it now. */
	const resetPalette = () => {
		setPalette(null);
		if (path && !busyRef.current) void run(path, { ...opts, customPalette: null, targetNodes, brushStrokes: strokes });
	};

	const apply = () => {
		if (path) void run(path, { ...opts, customPalette: palette, targetNodes, brushStrokes: strokes });
	};

	const choose = async () => {
		const p = await pickImage();
		if (p) openPicture(p);
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

	// ---- palette editing (the change waits for "Terapkan ulang")
	const currentPalette = (): string[] => palette ?? result?.palette ?? [];
	const removeColor = (hex: string) => {
		const base = currentPalette();
		if (base.length <= 2) return;
		setPalette(base.filter(c => c !== hex));
	};
	/** Merge into the nearest other colour: removing a colour maps its pixels to the nearest colour that is left. */
	const nearestOf = (hex: string): string | null => {
		const others = currentPalette().filter(c => c !== hex);
		if (others.length === 0) return null;
		const a = rgb(hex);
		let best: string | null = others[0] ?? null;
		let bd = Infinity;
		for (const o of others) {
			const b = rgb(o);
			const d = (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;
			if (d < bd) { bd = d; best = o; }
		}
		return best;
	};
	const addColor = (hex: string) => {
		const base = currentPalette();
		if (base.includes(hex)) { setInfo(`Warna ${hex} sudah ada di palet.`); return; }
		setPalette([...base, hex]);
		setInfo(`Warna ${hex} ditambahkan. Klik Terapkan ulang untuk memakainya.`);
	};

	// ---- painting on the preview
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
		if (s) setStrokes(list => [...list, s]);
	};

	const dirty = result !== null && appliedSig !== "" && JSON.stringify({ ...opts, customPalette: palette, targetNodes, brushStrokes: strokes }) !== appliedSig;
	const paletteDirty = result !== null && palette !== null && JSON.stringify(palette) !== JSON.stringify(result.palette);
	const painting = tool === "detail" || tool === "smooth";
	const shown = live ? [...strokes, live] : strokes;

	return (
		<div className={`app ${over ? "dragging" : ""}`} data-signature={signature.length}>
			<header className="top">
				<div className="brand"><span className="logo" aria-hidden="true">T</span> Trasea <small>0.4.0 · offline</small></div>
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
					{result && !busy && (
						<div className="stagebox">
							<div className="toolbar" role="toolbar" aria-label="Alat kanvas">
								<div className="group" role="group" aria-label="Mode">
									<button aria-pressed={tool === "pan"} onClick={() => setTool("pan")}>Geser</button>
									<button aria-pressed={tool === "detail"} onClick={() => setTool("detail")} className="tool-detail">Kuas Detail</button>
									<button aria-pressed={tool === "smooth"} onClick={() => setTool("smooth")} className="tool-smooth">Kuas Halus</button>
									<button aria-pressed={tool === "pipet"} onClick={() => setTool("pipet")}>Pipet Warna</button>
								</div>
								{painting && (
									<div className="group">
										<label className="brushsize" htmlFor="brush">Ukuran <b>{brushSize}</b> px</label>
										<input id="brush" type="range" min={10} max={120} value={brushSize} onChange={e => setBrushSize(Number(e.target.value))} />
										<button onClick={() => setStrokes(s => s.slice(0, -1))} disabled={strokes.length === 0}>Undo</button>
										<button onClick={() => setStrokes([])} disabled={strokes.length === 0}>Bersihkan sapuan</button>
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
								<img className="layer" src={result.original} alt="Gambar asli" draggable={false} />
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
					<h2>Pengaturan</h2>
					<label htmlFor="colors">Jumlah warna <b>{opts.colors}</b></label>
					<input id="colors" type="range" min={2} max={16} value={opts.colors} onChange={e => set({ colors: Number(e.target.value) })} />
					<label htmlFor="smooth">Kehalusan <b>{opts.smooth < 34 ? "detail" : opts.smooth > 66 ? "halus" : "sedang"}</b></label>
					<input id="smooth" type="range" min={0} max={100} value={opts.smooth} onChange={e => set({ smooth: Number(e.target.value) })} />
					<div className="hint">Kiri: lebih banyak detail dan node. Kanan: lebih sedikit node.</div>
					<label htmlFor="target">Target node (maks)</label>
					<select id="target" value={targetChoice} onChange={e => setTargetChoice(e.target.value as TargetChoice)}>
						<option value="auto">Otomatis</option>
						<option value="500">500</option>
						<option value="1000">1000</option>
						<option value="2000">2000</option>
						<option value="custom">Angka sendiri…</option>
					</select>
					{targetChoice === "custom" && <input className="num" type="number" min={1} step={50} value={customTarget} onChange={e => setCustomTarget(e.target.value)} aria-label="Target node" />}
					<div className="hint">Trasea menghaluskan hasil sampai jumlah node masuk anggaran, tanpa membersihkan gambar ulang.</div>
					<label className="check"><input type="checkbox" checked={opts.keepLines} onChange={e => set({ keepLines: e.target.checked })} /> Jaga garis tipis</label>
					<label className="check"><input type="checkbox" checked={opts.detectPrimitives} onChange={e => set({ detectPrimitives: e.target.checked })} /> Kenali bentuk dasar</label>
					<div className="hint">Lingkaran, elips, dan persegi yang pas ditulis sebagai bentuk murni, bukan kurva perkiraan.</div>
					<label htmlFor="symmetry">Tegas simetri</label>
					<select id="symmetry" value={opts.symmetry} onChange={e => set({ symmetry: e.target.value as Options["symmetry"] })}>
						<option value="off">Mati</option>
						<option value="auto">Otomatis</option>
						<option value="vertical">Vertikal (kiri = kanan)</option>
						<option value="horizontal">Horizontal (atas = bawah)</option>
					</select>
					<div className="hint">Untuk logo yang seharusnya simetris. Satu sisi disalin ke sisi lain.</div>
					<label className="check"><input type="checkbox" checked={opts.aiClean} onChange={e => set({ aiClean: e.target.checked })} /> Bersihkan dengan AI</label>
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
									<li key={c} className="swatch">
										<span className="chip" style={{ background: c }} aria-hidden="true" />
										<code>{c}</code>
										<button className="small" title="Gabung ke warna terdekat" onClick={() => removeColor(c)} disabled={currentPalette().length <= 2 || nearestOf(c) === null} aria-label={`Gabung ${c} ke ${nearestOf(c) ?? "-"}`}>Gabung</button>
										<button className="small" onClick={() => removeColor(c)} disabled={currentPalette().length <= 2} aria-label={`Hapus ${c}`}>Hapus</button>
									</li>
								))}
							</ul>
							<div className="hint">Gabung dan Hapus memindahkan piksel warna itu ke warna terdekat yang tersisa (minimal 2 warna). {paletteDirty ? "Palet berubah, klik Terapkan ulang." : palette !== null ? "Palet manual dipakai." : "Palet otomatis."}</div>

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
