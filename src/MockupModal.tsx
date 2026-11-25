import { useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { pickMockupPngSavePath, saveMockupPng } from "./api";
import type { Dict } from "./i18n/id";
import cardDark from "./assets/mockups/card-dark.webp";
import cardLight from "./assets/mockups/card-light.webp";
import stickerSurface from "./assets/mockups/sticker-surface.webp";
import totebagLight from "./assets/mockups/totebag-light.webp";
import tshirtDark from "./assets/mockups/tshirt-dark.webp";
import tshirtLight from "./assets/mockups/tshirt-light.webp";

export type MockupPreset = "tshirt" | "sticker" | "tote" | "card" | "appIcon";
export type MockupSurface = "light" | "dark";

interface MockupModalProps {
	svg: string;
	baseName: string;
	t: Record<keyof Dict, string>;
	onClose: () => void;
	onSaved: (path: string) => void;
}

interface SafeRect {
	/** Percent of 4:3 stage */
	x: number;
	y: number;
	w: number;
	h: number;
}

const SAFE_RECTS: Record<MockupPreset, SafeRect> = {
	tshirt: { x: 35, y: 27, w: 30, h: 38 },
	tote: { x: 33, y: 41, w: 34, h: 40 },
	card: { x: 28, y: 33, w: 44, h: 34 },
	sticker: { x: 26, y: 18, w: 48, h: 64 },
	appIcon: { x: 33, y: 20, w: 34, h: 60 },
};

function getBackdropUrl(preset: MockupPreset, surface: MockupSurface): string | null {
	if (preset === "tshirt") return surface === "dark" ? tshirtDark : tshirtLight;
	if (preset === "tote") return totebagLight;
	if (preset === "card") return surface === "dark" ? cardDark : cardLight;
	if (preset === "sticker") return stickerSurface;
	return null;
}

function svgToDataUrl(svg: string): string {
	const cleaned = svg.includes("xmlns=")
		? svg
		: svg.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"');
	return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(cleaned)}`;
}

function loadImage(src: string): Promise<HTMLImageElement> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve(img);
		img.onerror = () => reject(new Error("Image load failed"));
		img.src = src;
	});
}

function drawRoundedRect(
	ctx: CanvasRenderingContext2D,
	x: number,
	y: number,
	w: number,
	h: number,
	r: number,
) {
	ctx.beginPath();
	ctx.moveTo(x + r, y);
	ctx.arcTo(x + w, y, x + w, y + h, r);
	ctx.arcTo(x + w, y + h, x, y + h, r);
	ctx.arcTo(x, y + h, x, y, r);
	ctx.arcTo(x, y, x + w, y, r);
	ctx.closePath();
}

async function renderMockupToPngDataUrl(
	svgUrl: string,
	preset: MockupPreset,
	surface: MockupSurface,
	scalePct: number,
	offsetX: number,
	offsetY: number,
): Promise<string> {
	const W = 1600;
	const H = 1200;
	const canvas = document.createElement("canvas");
	canvas.width = W;
	canvas.height = H;
	const ctx = canvas.getContext("2d");
	if (!ctx) throw new Error("Canvas context unavailable");

	const artImg = await loadImage(svgUrl);
	const bgUrl = getBackdropUrl(preset, surface);
	const bgImg = bgUrl ? await loadImage(bgUrl) : null;
	const dark = surface === "dark";

	// 1. Studio backdrop photo or App Icon gradient studio
	if (bgImg) {
		ctx.save();
		if (preset === "tote" && dark) {
			ctx.filter = "brightness(0.36) contrast(1.25) saturate(0.45)";
		}
		ctx.drawImage(bgImg, 0, 0, W, H);
		ctx.restore();
	} else {
		const grad = ctx.createLinearGradient(0, 0, W, H);
		if (dark) {
			grad.addColorStop(0, "#10141e");
			grad.addColorStop(1, "#1c2233");
		} else {
			grad.addColorStop(0, "#eef2f9");
			grad.addColorStop(1, "#d9e2f0");
		}
		ctx.fillStyle = grad;
		ctx.fillRect(0, 0, W, H);
	}

	// 2. Compute exact placement box from calibrated safeRect + scale + user drag offset
	const rect = SAFE_RECTS[preset];
	const bx = (rect.x / 100) * W + (offsetX / 100) * W;
	const by = (rect.y / 100) * H + (offsetY / 100) * H;
	const bw = (rect.w / 100) * W;
	const bh = (rect.h / 100) * H;
	const k = Math.max(0.3, Math.min(1.0, scalePct / 100));
	const iw = artImg.naturalWidth || 512;
	const ih = artImg.naturalHeight || 512;
	const fit = Math.min((bw * k) / iw, (bh * k) / ih);
	const dw = iw * fit;
	const dh = ih * fit;
	const dx = bx + (bw - dw) / 2;
	const dy = by + (bh - dh) / 2;

	if (preset === "appIcon") {
		const tile = 580;
		const tx = (W - tile) / 2;
		const ty = (H - tile) / 2;
		const r = tile * 0.225;

		// 3-layer ambient shadow
		ctx.save();
		ctx.shadowColor = dark ? "rgba(0, 0, 0, 0.72)" : "rgba(15, 23, 42, 0.26)";
		ctx.shadowBlur = 72;
		ctx.shadowOffsetY = 34;
		drawRoundedRect(ctx, tx, ty, tile, tile, r);
		const tileGrad = ctx.createLinearGradient(tx, ty, tx, ty + tile);
		if (dark) {
			tileGrad.addColorStop(0, "#232838");
			tileGrad.addColorStop(1, "#141822");
		} else {
			tileGrad.addColorStop(0, "#ffffff");
			tileGrad.addColorStop(1, "#edf1f8");
		}
		ctx.fillStyle = tileGrad;
		ctx.fill();
		ctx.restore();

		// Clip inside squircle, draw artwork + soft top glass highlight
		ctx.save();
		drawRoundedRect(ctx, tx, ty, tile, tile, r);
		ctx.clip();
		const iconFit = Math.min((tile * 0.76 * k) / iw, (tile * 0.76 * k) / ih);
		const idw = iw * iconFit;
		const idh = ih * iconFit;
		const idx = tx + (tile - idw) / 2 + (offsetX / 100) * W;
		const idy = ty + (tile - idh) / 2 + (offsetY / 100) * H;
		ctx.drawImage(artImg, idx, idy, idw, idh);

		const glass = ctx.createLinearGradient(tx, ty, tx, ty + tile * 0.52);
		glass.addColorStop(0, "rgba(255,255,255,0.22)");
		glass.addColorStop(1, "rgba(255,255,255,0)");
		ctx.fillStyle = glass;
		ctx.fillRect(tx, ty, tile, tile * 0.52);
		ctx.restore();
	} else if (preset === "sticker") {
		// Thick white die-cut vinyl border + soft contact shadow + diagonal gloss
		const pad = 34;
		const off = document.createElement("canvas");
		off.width = Math.ceil(dw + pad * 2);
		off.height = Math.ceil(dh + pad * 2);
		const octx = off.getContext("2d");
		if (octx) {
			// Build solid white silhouette expanded in 16 directions
			const sil = document.createElement("canvas");
			sil.width = off.width;
			sil.height = off.height;
			const sctx = sil.getContext("2d")!;
			sctx.drawImage(artImg, pad, pad, dw, dh);
			sctx.globalCompositeOperation = "source-in";
			sctx.fillStyle = "#ffffff";
			sctx.fillRect(0, 0, sil.width, sil.height);

			const rad = 18;
			for (let a = 0; a < 20; a++) {
				const ang = (a / 20) * Math.PI * 2;
				octx.drawImage(sil, Math.cos(ang) * rad, Math.sin(ang) * rad);
			}
			octx.drawImage(sil, 0, 0);
			octx.drawImage(artImg, pad, pad, dw, dh);

			// Diagonal glossy vinyl highlight
			octx.save();
			octx.globalCompositeOperation = "source-atop";
			const gloss = octx.createLinearGradient(0, 0, off.width, off.height);
			gloss.addColorStop(0, "rgba(255,255,255,0.0)");
			gloss.addColorStop(0.42, "rgba(255,255,255,0.0)");
			gloss.addColorStop(0.5, "rgba(255,255,255,0.20)");
			gloss.addColorStop(0.58, "rgba(255,255,255,0.0)");
			gloss.addColorStop(1, "rgba(255,255,255,0.0)");
			octx.fillStyle = gloss;
			octx.fillRect(0, 0, off.width, off.height);
			octx.restore();

			ctx.save();
			ctx.shadowColor = "rgba(0, 0, 0, 0.48)";
			ctx.shadowBlur = 32;
			ctx.shadowOffsetY = 16;
			ctx.drawImage(off, dx - pad, dy - pad);
			ctx.restore();
		}
	} else {
		// T-Shirt / Tote Bag / Business Card: natural ink-to-fabric/paper blending
		const useMultiply = !dark;
		ctx.save();
		if (useMultiply) {
			ctx.globalCompositeOperation = "multiply";
			ctx.globalAlpha = 0.93;
		} else {
			ctx.globalAlpha = 0.96;
		}
		ctx.drawImage(artImg, dx, dy, dw, dh);
		ctx.restore();

		// On dark fabric/paper, modulate artwork with a subtle soft-light pass of the studio texture
		if (dark && bgImg) {
			ctx.save();
			ctx.beginPath();
			ctx.rect(dx, dy, dw, dh);
			ctx.clip();
			ctx.globalCompositeOperation = "soft-light";
			ctx.globalAlpha = 0.24;
			ctx.drawImage(bgImg, 0, 0, W, H);
			ctx.restore();
		}
	}

	return canvas.toDataURL("image/png");
}

export function MockupModal({ svg, baseName, t, onClose, onSaved }: MockupModalProps) {
	const [preset, setPreset] = useState<MockupPreset>("tshirt");
	const [surface, setSurface] = useState<MockupSurface>("light");
	const [scalePct, setScalePct] = useState<number>(74);
	const [offset, setOffset] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
	const [dragging, setDragging] = useState(false);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const stageRef = useRef<HTMLDivElement | null>(null);
	const dragStartRef = useRef<{ px: number; py: number; ox: number; oy: number } | null>(null);
	const svgUrl = useMemo(() => svgToDataUrl(svg), [svg]);

	const presets: Array<{ id: MockupPreset; label: string }> = [
		{ id: "tshirt", label: t.mockupTshirt },
		{ id: "sticker", label: t.mockupSticker },
		{ id: "tote", label: t.mockupTote },
		{ id: "card", label: t.mockupCard },
		{ id: "appIcon", label: t.mockupAppIcon },
	];

	const selectPreset = (next: MockupPreset) => {
		setPreset(next);
		setOffset({ x: 0, y: 0 });
	};

	const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
		e.currentTarget.setPointerCapture(e.pointerId);
		setDragging(true);
		dragStartRef.current = { px: e.clientX, py: e.clientY, ox: offset.x, oy: offset.y };
	};

	const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
		if (!dragging || !dragStartRef.current || !stageRef.current) return;
		const rect = stageRef.current.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) return;
		const dxPct = ((e.clientX - dragStartRef.current.px) / rect.width) * 100;
		const dyPct = ((e.clientY - dragStartRef.current.py) / rect.height) * 100;
		setOffset({
			x: Math.max(-35, Math.min(35, dragStartRef.current.ox + dxPct)),
			y: Math.max(-35, Math.min(35, dragStartRef.current.oy + dyPct)),
		});
	};

	const onPointerUp = () => {
		setDragging(false);
		dragStartRef.current = null;
	};

	const doSavePng = async () => {
		setError(null);
		try {
			const dest = await pickMockupPngSavePath(`${baseName}-${preset}-mockup.png`);
			if (!dest) return;
			setSaving(true);
			const dataUrl = await renderMockupToPngDataUrl(
				svgUrl,
				preset,
				surface,
				scalePct,
				offset.x,
				offset.y,
			);
			await saveMockupPng(dest, dataUrl);
			onSaved(dest);
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		} finally {
			setSaving(false);
		}
	};

	const bgUrl = getBackdropUrl(preset, surface);
	const dark = surface === "dark";
	const safe = SAFE_RECTS[preset];
	const isLightBlend = !dark && (preset === "tshirt" || preset === "tote" || preset === "card");
	const isDarkBlend = dark && (preset === "tshirt" || preset === "tote" || preset === "card");
	const hasOffset = Math.abs(offset.x) > 0.2 || Math.abs(offset.y) > 0.2 || scalePct !== 74;

	return (
		<div className="modal-backdrop" role="dialog" aria-modal="true" aria-label={t.mockupTitle}>
			<div className="modal-card mockup-modal">
				<div className="modal-head">
					<h3>{t.mockupTitle}</h3>
					<button className="small" onClick={onClose}>{t.close}</button>
				</div>

				<div className="mockup-controls">
					<div className="mockup-presets" role="tablist" aria-label={t.mockupTitle}>
						{presets.map(p => (
							<button
								key={p.id}
								role="tab"
								aria-selected={preset === p.id}
								className={preset === p.id ? "primary" : ""}
								onClick={() => selectPreset(p.id)}
							>
								{p.label}
							</button>
						))}
					</div>

					<div className="mockup-bar-right">
						<div className="seg-group" role="group" aria-label={t.mockupSurfaceLabel}>
							<button
								className={surface === "light" ? "active" : ""}
								onClick={() => setSurface("light")}
							>
								{t.mockupLight}
							</button>
							<button
								className={surface === "dark" ? "active" : ""}
								onClick={() => setSurface("dark")}
							>
								{t.mockupDark}
							</button>
						</div>

						<label className="mockup-scale">
							<span>{t.mockupScaleLabel}</span>
							<input
								type="range"
								min={30}
								max={100}
								value={scalePct}
								onChange={e => setScalePct(Number(e.target.value))}
							/>
							<b className="tabular">{scalePct}%</b>
						</label>

						<button
							className="small"
							disabled={!hasOffset}
							onClick={() => {
								setOffset({ x: 0, y: 0 });
								setScalePct(74);
							}}
						>
							{t.mockupResetPos}
						</button>
					</div>
				</div>

				{/* SVG filter for realistic white die-cut vinyl border + contact shadow */}
				<svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true">
					<defs>
						<filter id="trasea-diecut-vinyl" x="-25%" y="-25%" width="150%" height="150%">
							<feMorphology in="SourceAlpha" operator="dilate" radius="10" result="expanded" />
							<feFlood floodColor="#ffffff" result="whiteFill" />
							<feComposite in="whiteFill" in2="expanded" operator="in" result="whiteBorder" />
							<feDropShadow dx="0" dy="8" stdDeviation="10" floodColor="#000000" floodOpacity="0.45" result="shadowed" />
							<feMerge>
								<feMergeNode in="shadowed" />
								<feMergeNode in="whiteBorder" />
								<feMergeNode in="SourceGraphic" />
							</feMerge>
						</filter>
					</defs>
				</svg>

				<div className="mockup-stage-wrap">
					<div
						ref={stageRef}
						className={`mockup-photo-stage ${dark ? "surface-dark" : "surface-light"} ${dragging ? "is-dragging" : ""}`}
						onPointerDown={onPointerDown}
						onPointerMove={onPointerMove}
						onPointerUp={onPointerUp}
						onPointerCancel={onPointerUp}
					>
						{bgUrl && (
							<img
								src={bgUrl}
								alt=""
								draggable={false}
								className={`mockup-bg-photo ${preset === "tote" && dark ? "tote-dark-filter" : ""}`}
							/>
						)}

						{preset === "appIcon" ? (
							<div className={`mockup-squircle ${dark ? "dark" : "light"}`}>
								<img
									src={svgUrl}
									alt={baseName}
									draggable={false}
									style={{
										transform: `translate(${offset.x * 2}%, ${offset.y * 2}%) scale(${scalePct / 100})`,
									}}
								/>
								<div className="mockup-squircle-glass" aria-hidden="true" />
							</div>
						) : (
							<div
								className={`mockup-print-zone ${isLightBlend ? "blend-light" : ""} ${preset === "sticker" ? "is-sticker" : ""}`}
								style={{
									left: `${safe.x + offset.x}%`,
									top: `${safe.y + offset.y}%`,
									width: `${safe.w}%`,
									height: `${safe.h}%`,
								}}
							>
								<img
									src={svgUrl}
									alt={baseName}
									draggable={false}
									style={{ transform: `scale(${scalePct / 100})` }}
								/>
								{preset === "sticker" && <div className="mockup-sticker-gloss" aria-hidden="true" />}
							</div>
						)}

						{/* Subtle fabric/paper fold texture pass over dark materials */}
						{isDarkBlend && bgUrl && (
							<img
								src={bgUrl}
								alt=""
								draggable={false}
								className="mockup-dark-texture-overlay"
								aria-hidden="true"
							/>
						)}
					</div>
				</div>

				{error && <div className="error-banner">{error}</div>}

				<div className="modal-foot">
					<span className="muted tabular">{t.mockupDragHint} · PNG 1600×1200 px</span>
					<div className="row">
						<button onClick={onClose}>{t.close}</button>
						<button className="primary" onClick={() => void doSavePng()} disabled={saving}>
							{saving ? t.savingLabel : t.mockupSavePng}
						</button>
					</div>
				</div>
			</div>
		</div>
	);
}
