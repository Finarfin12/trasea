import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { loadRawImagePreview, type PreprocessOptions, type RawImagePreview } from "./api";
import { useI18n } from "./i18n";

interface Props {
	path: string;
	initial: PreprocessOptions;
	onCancel: () => void;
	onApply: (next: PreprocessOptions) => void;
}

type DragMode = null | "move" | "nw" | "ne" | "sw" | "se";

function normalizeDeg(deg: number): number {
	let a = deg % 360;
	if (a > 180) a -= 360;
	if (a <= -180) a += 360;
	return Math.round(a * 10) / 10;
}

function splitAngle(deg: number): { quarter: number; fine: number } {
	const n = normalizeDeg(deg);
	const quarters = [-180, -90, 0, 90, 180];
	let bestQ = 0;
	let bestDiff = 999;
	for (const q of quarters) {
		const d = Math.abs(n - q);
		if (d < bestDiff) {
			bestDiff = d;
			bestQ = q === -180 ? 180 : q;
		}
	}
	let fine = normalizeDeg(n - bestQ);
	if (fine > 45) fine = 45;
	if (fine < -45) fine = -45;
	return { quarter: bestQ, fine };
}

function rotatedDims(w: number, h: number, deg: number): [number, number] {
	const n = normalizeDeg(deg);
	if (Math.abs(n) < 1e-3 || Math.abs(Math.abs(n) - 180) < 1e-3) return [w, h];
	if (Math.abs(Math.abs(n) - 90) < 1e-3) return [h, w];
	const rad = (n * Math.PI) / 180;
	const s = Math.abs(Math.sin(rad));
	const c = Math.abs(Math.cos(rad));
	return [Math.max(1, Math.ceil(w * c + h * s)), Math.max(1, Math.ceil(w * s + h * c))];
}

export default function CropModal({ path, initial, onCancel, onApply }: Props) {
	const { t } = useI18n();
	const initSplit = useMemo(() => splitAngle(initial.rotateDeg || 0), [initial.rotateDeg]);
	const [preview, setPreview] = useState<RawImagePreview | null>(null);
	const [imgEl, setImgEl] = useState<HTMLImageElement | null>(null);
	const [err, setErr] = useState("");
	const [quarter, setQuarter] = useState<number>(initSplit.quarter);
	const [fine, setFine] = useState<number>(initSplit.fine);
	const [crop, setCrop] = useState<[number, number, number, number] | null>(initial.crop);

	const canvasRef = useRef<HTMLCanvasElement | null>(null);
	const dragRef = useRef<{
		mode: DragMode;
		startX: number;
		startY: number;
		orig: [number, number, number, number];
	} | null>(null);

	const totalDeg = useMemo(() => normalizeDeg(quarter + fine), [quarter, fine]);

	useEffect(() => {
		let alive = true;
		setErr("");
		void loadRawImagePreview(path)
			.then(res => {
				if (!alive) return;
				setPreview(res);
				const im = new Image();
				im.onload = () => {
					if (alive) setImgEl(im);
				};
				im.src = res.dataUrl;
			})
			.catch(e => {
				if (alive) setErr(typeof e === "string" ? e : e instanceof Error ? e.message : t.unknownError);
			});
		return () => {
			alive = false;
		};
	}, [path, t.unknownError]);

	const [rotW, rotH] = useMemo(() => {
		if (!preview) return [1, 1];
		return rotatedDims(preview.width, preview.height, totalDeg);
	}, [preview, totalDeg]);

	// Keep crop clamped to rotated bounds whenever image loads or angle changes
	useEffect(() => {
		if (!preview) return;
		setCrop(prev => {
			if (!prev) return [0, 0, rotW, rotH];
			const x = Math.max(0, Math.min(rotW - 10, prev[0]));
			const y = Math.max(0, Math.min(rotH - 10, prev[1]));
			const w = Math.max(10, Math.min(rotW - x, prev[2]));
			const h = Math.max(10, Math.min(rotH - y, prev[3]));
			return [Math.round(x), Math.round(y), Math.round(w), Math.round(h)];
		});
	}, [preview, rotW, rotH]);

	const effCrop: [number, number, number, number] = crop ?? [0, 0, rotW, rotH];

	// Render rotated image + interactive crop box on canvas
	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas || !imgEl || !preview) return;
		const maxW = 680;
		const maxH = 400;
		const scale = Math.min(maxW / rotW, maxH / rotH, 1);
		const cw = Math.max(1, Math.round(rotW * scale));
		const ch = Math.max(1, Math.round(rotH * scale));
		canvas.width = cw;
		canvas.height = ch;

		const ctx = canvas.getContext("2d");
		if (!ctx) return;
		ctx.clearRect(0, 0, cw, ch);
		ctx.fillStyle = "#111318";
		ctx.fillRect(0, 0, cw, ch);

		ctx.save();
		ctx.translate(cw / 2, ch / 2);
		ctx.rotate((totalDeg * Math.PI) / 180);
		ctx.drawImage(imgEl, (-preview.width * scale) / 2, (-preview.height * scale) / 2, preview.width * scale, preview.height * scale);
		ctx.restore();

		const [cx, cy, bw, bh] = effCrop;
		const rx = cx * scale;
		const ry = cy * scale;
		const rw = bw * scale;
		const rh = bh * scale;

		// Dim exterior
		ctx.fillStyle = "rgba(0, 0, 0, 0.55)";
		ctx.fillRect(0, 0, cw, ry);
		ctx.fillRect(0, ry + rh, cw, ch - (ry + rh));
		ctx.fillRect(0, ry, rx, rh);
		ctx.fillRect(rx + rw, ry, cw - (rx + rw), rh);

		// Crop frame + rule-of-thirds grid
		ctx.strokeStyle = "#6d8cff";
		ctx.lineWidth = 2;
		ctx.strokeRect(rx + 1, ry + 1, Math.max(1, rw - 2), Math.max(1, rh - 2));

		ctx.strokeStyle = "rgba(255, 255, 255, 0.25)";
		ctx.lineWidth = 1;
		for (let i = 1; i <= 2; i++) {
			ctx.beginPath();
			ctx.moveTo(rx + (rw * i) / 3, ry);
			ctx.lineTo(rx + (rw * i) / 3, ry + rh);
			ctx.stroke();
			ctx.beginPath();
			ctx.moveTo(rx, ry + (rh * i) / 3);
			ctx.lineTo(rx + rw, ry + (rh * i) / 3);
			ctx.stroke();
		}

		// Corner handles
		const hs = 10;
		const corners: Array<[number, number]> = [
			[rx, ry],
			[rx + rw, ry],
			[rx, ry + rh],
			[rx + rw, ry + rh],
		];
		ctx.fillStyle = "#6d8cff";
		ctx.strokeStyle = "#ffffff";
		ctx.lineWidth = 1.5;
		for (const [hx, hy] of corners) {
			ctx.fillRect(hx - hs / 2, hy - hs / 2, hs, hs);
			ctx.strokeRect(hx - hs / 2, hy - hs / 2, hs, hs);
		}
	}, [imgEl, preview, rotW, rotH, totalDeg, effCrop]);

	const canvasToRot = (e: ReactPointerEvent<HTMLCanvasElement>): [number, number] => {
		const rect = e.currentTarget.getBoundingClientRect();
		const x = ((e.clientX - rect.left) / Math.max(1, rect.width)) * rotW;
		const y = ((e.clientY - rect.top) / Math.max(1, rect.height)) * rotH;
		return [x, y];
	};

	const hitTest = (x: number, y: number): DragMode => {
		const [cx, cy, bw, bh] = effCrop;
		const pad = Math.max(12, Math.max(rotW, rotH) * 0.03);
		if (Math.hypot(x - cx, y - cy) <= pad) return "nw";
		if (Math.hypot(x - (cx + bw), y - cy) <= pad) return "ne";
		if (Math.hypot(x - cx, y - (cy + bh)) <= pad) return "sw";
		if (Math.hypot(x - (cx + bw), y - (cy + bh)) <= pad) return "se";
		if (x >= cx && x <= cx + bw && y >= cy && y <= cy + bh) return "move";
		return null;
	};

	const onDown = (e: ReactPointerEvent<HTMLCanvasElement>) => {
		if (!preview || e.button !== 0) return;
		const [x, y] = canvasToRot(e);
		const mode = hitTest(x, y);
		if (!mode) return;
		e.currentTarget.setPointerCapture(e.pointerId);
		dragRef.current = { mode, startX: x, startY: y, orig: [...effCrop] };
	};

	const onMove = (e: ReactPointerEvent<HTMLCanvasElement>) => {
		const d = dragRef.current;
		if (!d) return;
		const [x, y] = canvasToRot(e);
		const dx = x - d.startX;
		const dy = y - d.startY;
		let [ox, oy, ow, oh] = d.orig;
		const minSize = Math.min(24, rotW, rotH);

		if (d.mode === "move") {
			ox = Math.max(0, Math.min(rotW - ow, ox + dx));
			oy = Math.max(0, Math.min(rotH - oh, oy + dy));
		} else {
			let left = ox;
			let top = oy;
			let right = ox + ow;
			let bottom = oy + oh;
			if (d.mode === "nw" || d.mode === "sw") left = Math.max(0, Math.min(right - minSize, ox + dx));
			if (d.mode === "ne" || d.mode === "se") right = Math.min(rotW, Math.max(left + minSize, ox + ow + dx));
			if (d.mode === "nw" || d.mode === "ne") top = Math.max(0, Math.min(bottom - minSize, oy + dy));
			if (d.mode === "sw" || d.mode === "se") bottom = Math.min(rotH, Math.max(top + minSize, oy + oh + dy));
			ox = left;
			oy = top;
			ow = right - left;
			oh = bottom - top;
		}
		setCrop([Math.round(ox), Math.round(oy), Math.round(ow), Math.round(oh)]);
	};

	const onUp = () => {
		dragRef.current = null;
	};

	const rotateBy = (delta: number) => {
		setQuarter(q => {
			const next = normalizeDeg(q + delta);
			return next === -180 ? 180 : next;
		});
		setCrop(null);
	};

	const doReset = () => {
		setQuarter(0);
		setFine(0);
		setCrop(null);
	};

	const isFullCrop = effCrop[0] <= 1 && effCrop[1] <= 1 && effCrop[2] >= rotW - 2 && effCrop[3] >= rotH - 2;

	const doApply = () => {
		onApply({
			crop: isFullCrop ? null : effCrop,
			rotateDeg: Math.abs(totalDeg) < 0.05 ? 0 : totalDeg,
		});
	};

	return (
		<div className="modal" role="dialog" aria-modal="true" aria-label={t.cropModalTitle} onClick={onCancel}>
			<div className="modal-box crop-modal-box" onClick={e => e.stopPropagation()}>
				<div className="row between">
					<h2 style={{ margin: 0 }}>{t.cropModalTitle}</h2>
					<button className="ghost" onClick={onCancel}>{t.close}</button>
				</div>
				<div className="hint" style={{ marginTop: 4 }}>{t.cropModalHint}</div>

				<div className="crop-canvas-wrap">
					{err ? (
						<div className="error">{err}</div>
					) : !imgEl ? (
						<div className="muted">{t.cropLoading}</div>
					) : (
						<canvas
							ref={canvasRef}
							className="crop-canvas"
							onPointerDown={onDown}
							onPointerMove={onMove}
							onPointerUp={onUp}
							onPointerCancel={onUp}
						/>
					)}
				</div>

				<div className="crop-controls">
					<div className="row">
						<button className="small" onClick={() => rotateBy(-90)}>{t.cropRotLeft}</button>
						<button className="small" onClick={() => rotateBy(90)}>{t.cropRotRight}</button>
						<button className="small" onClick={() => rotateBy(180)}>{t.cropRot180}</button>
						<label className="crop-angle-label" htmlFor="fineAngle">
							{t.cropFineAngle}: <b>{fine > 0 ? `+${fine}°` : `${fine}°`}</b>
						</label>
						<input
							id="fineAngle"
							type="range"
							min={-45}
							max={45}
							step={0.5}
							value={fine}
							onChange={e => {
								setFine(Number(e.target.value));
								setCrop(null);
							}}
							style={{ width: 160, accentColor: "var(--accent)" }}
						/>
					</div>
					<div className="row between muted" style={{ marginTop: 6 }}>
						<span>
							{t.cropTotalAngle}: <b>{totalDeg}°</b> · {t.cropSelection}:{" "}
							<b>{isFullCrop ? `${t.cropFullImage} (${rotW} × ${rotH} px)` : `${effCrop[2]} × ${effCrop[3]} px (${effCrop[0]}, ${effCrop[1]})`}</b>
						</span>
						<div className="row">
							<button className="small" onClick={doReset}>{t.cropReset}</button>
							<button className="ghost" onClick={onCancel}>{t.cropCancel}</button>
							<button className="primary" style={{ width: "auto", margin: 0 }} onClick={doApply} disabled={!preview}>{t.cropApply}</button>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
