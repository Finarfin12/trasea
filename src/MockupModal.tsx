import { useMemo, useState } from "react";
import { pickMockupPngSavePath, saveMockupPng } from "./api";
import type { Dict } from "./i18n/id";

export type MockupPreset = "tshirt" | "sticker" | "tote" | "card" | "appIcon";
export type MockupSurface = "light" | "dark";

interface MockupModalProps {
	svg: string;
	baseName: string;
	t: Record<keyof Dict, string>;
	onClose: () => void;
	onSaved: (path: string) => void;
}

function svgToDataUrl(svg: string): string {
	const cleaned = svg.includes("xmlns=")
		? svg
		: svg.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"');
	return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(cleaned)}`;
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

function renderMockupCanvas(
	canvas: HTMLCanvasElement,
	img: HTMLImageElement,
	preset: MockupPreset,
	surface: MockupSurface,
	scalePct: number,
) {
	const W = 1600;
	const H = 1200;
	canvas.width = W;
	canvas.height = H;
	const ctx = canvas.getContext("2d");
	if (!ctx) return;

	const dark = surface === "dark";

	// Studio backdrop
	const bg = ctx.createLinearGradient(0, 0, W, H);
	if (dark) {
		bg.addColorStop(0, "#10131a");
		bg.addColorStop(1, "#1a1f2b");
	} else {
		bg.addColorStop(0, "#eef2f8");
		bg.addColorStop(1, "#dde4f0");
	}
	ctx.fillStyle = bg;
	ctx.fillRect(0, 0, W, H);

	// Soft radial studio spotlight
	const spot = ctx.createRadialGradient(W * 0.5, H * 0.45, 60, W * 0.5, H * 0.45, 760);
	spot.addColorStop(0, dark ? "rgba(109, 140, 255, 0.14)" : "rgba(255, 255, 255, 0.72)");
	spot.addColorStop(1, "rgba(0,0,0,0)");
	ctx.fillStyle = spot;
	ctx.fillRect(0, 0, W, H);

	// Product silhouette & placement box
	let box = { x: 560, y: 360, w: 480, h: 480 };
	ctx.save();
	ctx.shadowColor = dark ? "rgba(0,0,0,0.65)" : "rgba(15,23,42,0.20)";
	ctx.shadowBlur = 60;
	ctx.shadowOffsetY = 26;

	if (preset === "tshirt") {
		ctx.fillStyle = dark ? "#1f2430" : "#ffffff";
		ctx.strokeStyle = dark ? "rgba(255,255,255,0.12)" : "rgba(15,23,42,0.08)";
		ctx.lineWidth = 4;
		ctx.beginPath();
		ctx.moveTo(540, 175);
		ctx.quadraticCurveTo(800, 245, 1060, 175);
		ctx.lineTo(1280, 260);
		ctx.lineTo(1365, 505);
		ctx.lineTo(1170, 560);
		ctx.lineTo(1110, 415);
		ctx.lineTo(1110, 1045);
		ctx.lineTo(490, 1045);
		ctx.lineTo(490, 415);
		ctx.lineTo(430, 560);
		ctx.lineTo(235, 505);
		ctx.lineTo(320, 260);
		ctx.closePath();
		ctx.fill();
		ctx.stroke();
		box = { x: 580, y: 350, w: 440, h: 440 };
	} else if (preset === "sticker") {
		drawRoundedRect(ctx, 430, 230, 740, 740, 110);
		ctx.fillStyle = "#ffffff";
		ctx.fill();
		ctx.shadowBlur = 0;
		ctx.strokeStyle = "rgba(236, 0, 140, 0.35)";
		ctx.setLineDash([12, 10]);
		ctx.lineWidth = 4;
		drawRoundedRect(ctx, 452, 252, 696, 696, 94);
		ctx.stroke();
		ctx.setLineDash([]);
		box = { x: 520, y: 320, w: 560, h: 560 };
	} else if (preset === "tote") {
		// Handles
		ctx.strokeStyle = dark ? "#2a303f" : "#ebe6dc";
		ctx.lineWidth = 34;
		ctx.beginPath();
		ctx.arc(800, 320, 160, Math.PI, 0);
		ctx.stroke();
		// Bag body
		drawRoundedRect(ctx, 440, 320, 720, 730, 36);
		ctx.fillStyle = dark ? "#202531" : "#f4efe6";
		ctx.fill();
		ctx.lineWidth = 4;
		ctx.strokeStyle = dark ? "rgba(255,255,255,0.12)" : "rgba(15,23,42,0.08)";
		ctx.stroke();
		box = { x: 560, y: 455, w: 480, h: 480 };
	} else if (preset === "card") {
		drawRoundedRect(ctx, 300, 280, 1000, 620, 32);
		ctx.fillStyle = dark ? "#181c24" : "#ffffff";
		ctx.fill();
		ctx.lineWidth = 3;
		ctx.strokeStyle = dark ? "rgba(255,255,255,0.14)" : "rgba(15,23,42,0.09)";
		ctx.stroke();
		box = { x: 560, y: 360, w: 480, h: 460 };
	} else {
		// App icon squircle
		drawRoundedRect(ctx, 480, 280, 640, 640, 144);
		ctx.fillStyle = dark ? "#161a24" : "#ffffff";
		ctx.fill();
		ctx.lineWidth = 4;
		ctx.strokeStyle = dark ? "rgba(255,255,255,0.18)" : "rgba(15,23,42,0.10)";
		ctx.stroke();
		box = { x: 560, y: 360, w: 480, h: 480 };
	}
	ctx.restore();

	// Draw vector artwork centered inside placement box with scale slider
	const k = Math.max(0.3, Math.min(1.0, scalePct / 100));
	const maxW = box.w * k;
	const maxH = box.h * k;
	const iw = img.naturalWidth || 512;
	const ih = img.naturalHeight || 512;
	const fit = Math.min(maxW / iw, maxH / ih);
	const dw = iw * fit;
	const dh = ih * fit;
	const dx = box.x + (box.w - dw) / 2;
	const dy = box.y + (box.h - dh) / 2;
	ctx.drawImage(img, dx, dy, dw, dh);
}

export function MockupModal({ svg, baseName, t, onClose, onSaved }: MockupModalProps) {
	const [preset, setPreset] = useState<MockupPreset>("tshirt");
	const [surface, setSurface] = useState<MockupSurface>("dark");
	const [scalePct, setScalePct] = useState<number>(72);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const svgUrl = useMemo(() => svgToDataUrl(svg), [svg]);

	const presets: Array<{ id: MockupPreset; label: string }> = [
		{ id: "tshirt", label: t.mockupTshirt },
		{ id: "sticker", label: t.mockupSticker },
		{ id: "tote", label: t.mockupTote },
		{ id: "card", label: t.mockupCard },
		{ id: "appIcon", label: t.mockupAppIcon },
	];

	const doSavePng = async () => {
		setError(null);
		try {
			const dest = await pickMockupPngSavePath(`${baseName}-${preset}-mockup.png`);
			if (!dest) return;
			setSaving(true);
			const img = new Image();
			await new Promise<void>((resolve, reject) => {
				img.onload = () => resolve();
				img.onerror = () => reject(new Error("SVG render failed"));
				img.src = svgUrl;
			});
			const canvas = document.createElement("canvas");
			renderMockupCanvas(canvas, img, preset, surface, scalePct);
			const dataUrl = canvas.toDataURL("image/png");
			await saveMockupPng(dest, dataUrl);
			onSaved(dest);
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		} finally {
			setSaving(false);
		}
	};

	const dark = surface === "dark";

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
								onClick={() => setPreset(p.id)}
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
					</div>
				</div>

				<div className={`mockup-stage ${dark ? "surface-dark" : "surface-light"}`}>
					<div className={`mockup-object preset-${preset}`}>
						{preset === "tshirt" && (
							<svg className="mockup-shell-svg" viewBox="0 0 800 600" aria-hidden="true">
								<path
									d="M270,88 Q400,122 530,88 L640,130 L682,252 L585,280 L555,208 L555,522 L245,522 L245,208 L215,280 L118,252 L160,130 Z"
									fill={dark ? "#1f2430" : "#ffffff"}
									stroke={dark ? "rgba(255,255,255,0.14)" : "rgba(15,23,42,0.10)"}
									strokeWidth="2.5"
								/>
							</svg>
						)}
						{preset === "tote" && (
							<svg className="mockup-shell-svg" viewBox="0 0 800 600" aria-hidden="true">
								<path
									d="M320,170 A80,80 0 0,1 480,170"
									fill="none"
									stroke={dark ? "#2a303f" : "#e9e2d6"}
									strokeWidth="18"
								/>
								<rect
									x="220"
									y="165"
									width="360"
									height="365"
									rx="20"
									fill={dark ? "#202531" : "#f4efe6"}
									stroke={dark ? "rgba(255,255,255,0.14)" : "rgba(15,23,42,0.10)"}
									strokeWidth="2.5"
								/>
							</svg>
						)}
						{preset === "sticker" && (
							<div className="mockup-sticker-base" aria-hidden="true" />
						)}
						{preset === "card" && (
							<div className={`mockup-card-base ${dark ? "dark" : "light"}`} aria-hidden="true" />
						)}
						{preset === "appIcon" && (
							<div className={`mockup-icon-base ${dark ? "dark" : "light"}`} aria-hidden="true" />
						)}

						<div
							className={`mockup-art-slot slot-${preset}`}
							style={{ transform: `scale(${scalePct / 100})` }}
						>
							<img src={svgUrl} alt={baseName} draggable={false} />
						</div>
					</div>
				</div>

				{error && <div className="error-banner">{error}</div>}

				<div className="modal-foot">
					<span className="muted tabular">PNG 1600×1200 px</span>
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
