import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent } from "react";
import { createPortal } from "react-dom";
import { checkCmykGamut } from "./api";
import { hexToHsl, hslToHex, normalizeHexInput } from "./colorUtils";
import type { BrandPalettePreset } from "./BrandPalette";

interface Props {
	anchor: HTMLElement;
	value: string;
	documentPalette: string[];
	brandPalettes: BrandPalettePreset[];
	lang: "en" | "id";
	onLiveChange?: (hex: string | null) => void;
	onCommit: (hex: string) => void;
	onAddToPalette?: (hex: string) => void;
	onClose: () => void;
}

const labels = {
	en: { color: "Color", swatches: "Swatches", close: "Close color picker", recent: "Recent colors", document: "Document", brand: "Brand palettes", add: "+ Add to Palette", apply: "Apply color", gamut: "Outside estimated CMYK gamut", hue: "Hue", sat: "Saturation", light: "Lightness", safe: "Print Safe", greys: "Greys", basic: "Basic" },
	id: { color: "Warna", swatches: "Swatch", close: "Tutup pemilih warna", recent: "Warna terakhir", document: "Dokumen", brand: "Palet brand", add: "+ Tambah ke Palet", apply: "Terapkan warna", gamut: "Di luar perkiraan gamut CMYK", hue: "Rona", sat: "Saturasi", light: "Kecerahan", safe: "Aman Cetak", greys: "Abu-abu", basic: "Dasar" },
};

const presets = {
	basic: ["#F94144","#F3722C","#F9C74F","#90BE6D","#43AA8B","#4D908E","#277DA1","#4361EE","#8338EC","#D34B8A","#FFFFFF","#000000"],
	greys: ["#FFFFFF","#E5E5E5","#C5C5C5","#A6A6A6","#828282","#606060","#333333","#000000"],
	safe: ["#D7BAA0","#F0D8AE","#8C4738","#BD7965","#A59363","#657D53","#416959","#70918C","#536A89","#526178","#846B88","#4A414D"],
};

const recentKey = "trasea_recent_colors_v1";
const clamp = (n: number) => Math.max(0, Math.min(1, n));

function readRecent(): string[] {
	try {
		const saved: unknown = JSON.parse(localStorage.getItem(recentKey) ?? "[]");
		return Array.isArray(saved) ? [...new Set(saved.flatMap(item => typeof item === "string" ? normalizeHexInput(item) ?? [] : []))].slice(0, 8) : [];
	} catch {
		return [];
	}
}

export function ColorWheelPopover({ anchor, value, documentPalette, brandPalettes, lang, onLiveChange, onCommit, onAddToPalette, onClose }: Props) {
	const t = labels[lang];
	const [tab, setTab] = useState<"color" | "swatches">("color");
	const [hsl, setHsl] = useState(() => hexToHsl(value));
	const [draft, setDraft] = useState(() => normalizeHexInput(value) ?? "#000000");
	const [recent, setRecent] = useState(readRecent);
	const [outOfGamut, setOutOfGamut] = useState(false);
	const panel = useRef<HTMLDivElement>(null);
	const current = useRef(hsl);
	const frame = useRef<number | null>(null);
	const latest = useRef({ onClose, onLiveChange });
	latest.current = { onClose, onLiveChange };
	const color = hslToHex(hsl.h, hsl.s, hsl.l);

	useLayoutEffect(() => {
		const node = panel.current;
		if (!node) return;
		const place = () => {
			const rect = anchor.getBoundingClientRect();
			const { width, height } = node.getBoundingClientRect();
			node.style.left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8)) + "px";
			node.style.top = Math.max(8, Math.min(rect.bottom + 8, window.innerHeight - height - 8)) + "px";
		};
		place();
		const resize = new ResizeObserver(place);
		resize.observe(node);
		window.addEventListener("resize", place);
		window.addEventListener("scroll", place, true);
		return () => {
			resize.disconnect();
			window.removeEventListener("resize", place);
			window.removeEventListener("scroll", place, true);
		};
	}, [anchor]);

	useEffect(() => {
		const outside = (event: Event) => {
			if (!(event.target instanceof Node) || panel.current?.contains(event.target) || anchor.contains(event.target)) return;
			latest.current.onClose();
		};
		const escape = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.stopImmediatePropagation();
			event.preventDefault();
			latest.current.onClose();
			anchor.focus();
		};
		document.addEventListener("pointerdown", outside, true);
		window.addEventListener("keydown", escape, true);
		panel.current?.focus();
		return () => {
			document.removeEventListener("pointerdown", outside, true);
			window.removeEventListener("keydown", escape, true);
			if (frame.current !== null) cancelAnimationFrame(frame.current);
			latest.current.onLiveChange?.(null);
		};
	}, [anchor]);

	useEffect(() => {
		let active = true;
		const timer = window.setTimeout(() => void checkCmykGamut(color)
			.then(warn => { if (active) setOutOfGamut(warn); }).catch(() => {}), 120);
		return () => { active = false; clearTimeout(timer); };
	}, [color]);

	function update(next: typeof hsl) {
		current.current = next;
		setHsl(next);
		const hex = hslToHex(next.h, next.s, next.l);
		setDraft(hex);
		onLiveChange?.(hex);
	}

	function remember(hex: string) {
		const updated = [hex, ...recent.filter(item => item !== hex)].slice(0, 8);
		setRecent(updated);
		try { localStorage.setItem(recentKey, JSON.stringify(updated)); } catch { /* private storage may be unavailable */ }
	}

	function commit(hex: string) {
		remember(hex);
		onLiveChange?.(null);
		onCommit(hex);
	}

	function point(event: PointerEvent<HTMLDivElement>, mode: "hue" | "sl") {
		const r = event.currentTarget.getBoundingClientRect();
		const x = clamp((event.clientX - r.left) / r.width);
		const y = clamp((event.clientY - r.top) / r.height);
		return mode === "hue"
			? { ...current.current, h: (Math.atan2(x - 0.5, 0.5 - y) * 180 / Math.PI + 360) % 360 }
			: { ...current.current, s: x * 100, l: (1 - y) * 100 };
	}

	const drag = (mode: "hue" | "sl") => ({
		onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
			if (e.button !== 0) return;
			e.currentTarget.setPointerCapture(e.pointerId);
			update(point(e, mode));
		},
		onPointerMove: (e: PointerEvent<HTMLDivElement>) => {
			if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
			const next = point(e, mode);
			if (frame.current !== null) cancelAnimationFrame(frame.current);
			frame.current = requestAnimationFrame(() => { frame.current = null; update(next); });
		},
		onPointerUp: (e: PointerEvent<HTMLDivElement>) => {
			if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
			if (frame.current !== null) cancelAnimationFrame(frame.current);
			frame.current = null;
			const next = point(e, mode);
			update(next);
			commit(hslToHex(next.h, next.s, next.l));
		},
		onPointerCancel: () => {
			if (frame.current !== null) cancelAnimationFrame(frame.current);
			frame.current = null;
			onLiveChange?.(null);
		},
	});

	function chips(title: string, colors: string[]) {
		if (colors.length === 0) return null;
		return <div className="wheel-swatch-group"><strong>{title}</strong><div className="wheel-swatches">
			{colors.map(hex => <button key={hex} type="button" aria-label={hex} title={hex} style={{ background: hex }} onClick={() => { update(hexToHsl(hex)); commit(hex); }} />)}
		</div></div>;
	}

	return createPortal(<div className="wheel-popover" ref={panel} tabIndex={-1} role="dialog" aria-label={t.color}>
		<div className="wheel-header">
			<button type="button" aria-pressed={tab === "color"} onClick={() => setTab("color")}>{t.color}</button>
			<button type="button" aria-pressed={tab === "swatches"} onClick={() => setTab("swatches")}>{t.swatches}</button>
			<button className="wheel-close" type="button" aria-label={t.close} onClick={() => { onClose(); anchor.focus(); }}>×</button>
		</div>
		{tab === "color" ? <>
			<div className="wheel-disc">
				<div className="wheel-hue" aria-hidden="true" {...drag("hue")} />
				<div className="wheel-hue-cursor" aria-hidden="true" style={{ transform: `rotate(${hsl.h}deg) translateY(-85px)`, background: `hsl(${hsl.h} 100% 50%)` }} />
				<div className="wheel-sl" aria-hidden="true" style={{ backgroundColor: `hsl(${hsl.h} 100% 50%)` }} {...drag("sl")}>
					<span style={{ left: hsl.s + "%", top: 100 - hsl.l + "%" }} />
				</div>
			</div>
			<div className="wheel-readouts">
				{([["h", "H", t.hue, 359], ["s", "S", t.sat, 100], ["l", "L", t.light, 100]] as const).map(([key, label, title, max]) => <label key={key}>{label}
					<input type="number" min={0} max={max} value={Math.min(max, Math.round(hsl[key]))} aria-label={title} onChange={e => {
						update({ ...current.current, [key]: Math.min(max, Math.max(0, Number(e.target.value))) });
					}} />
				</label>)}
			</div>
			<form className="wheel-hex" onSubmit={e => { e.preventDefault(); const hex = normalizeHexInput(draft); if (hex) { update(hexToHsl(hex)); commit(hex); } }}>
				<span aria-hidden="true" style={{ background: color }} />
				<input aria-label="HEX" value={draft} maxLength={7} onChange={e => setDraft(e.target.value)} />
				<button type="submit" disabled={!normalizeHexInput(draft)}>{t.apply}</button>
			</form>
			{outOfGamut && <small className="wheel-gamut" role="status">{t.gamut}</small>}
			{chips(t.recent, recent)}
		</> : <>
			{chips(t.document, [...new Set(documentPalette)])}
			{chips(t.basic, presets.basic)}
			{chips(t.greys, presets.greys)}
			{chips(t.safe, presets.safe)}
			{brandPalettes.length > 0 && <div className="wheel-brand">
				<strong>{t.brand}</strong>
				{brandPalettes.map(preset => <div key={preset.id}>{chips(preset.name, preset.colors)}</div>)}
			</div>}
		</>}
		{onAddToPalette && <button type="button" className="wheel-add" onClick={() => {
			const hex = normalizeHexInput(draft);
			if (hex) { remember(hex); onLiveChange?.(null); onAddToPalette(hex); }
		}} disabled={!normalizeHexInput(draft)}>{t.add}</button>}
	</div>, document.body);
}
