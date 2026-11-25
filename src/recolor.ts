import type { RecolorSpec } from "./api";

function parseHex6(hex: string): [number, number, number] | null {
	const m = /^#([0-9a-fA-F]{6})$/.exec(hex.trim());
	if (!m || !m[1]) return null;
	const n = parseInt(m[1], 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function mapColor(hex: string, spec: RecolorSpec): string | null {
	const rgb = parseHex6(hex);
	if (!rgb) return null;
	// Never recolor CutContour spot magenta (#EC008C)
	if (rgb[0] === 0xec && rgb[1] === 0x00 && rgb[2] === 0x8c) return null;
	if (spec.mode === "monoBlack") return "#000000";
	if (spec.mode === "monoWhite") return "#FFFFFF";
	if (spec.mode === "duotone") {
		const lum = (0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]) / 255;
		return lum < 0.52 ? (spec.primary || "#111827") : (spec.secondary || "#6D8CFF");
	}
	return null;
}

/**
 * Instant (0 ms) non-destructive SVG recoloring in the browser preview.
 * Preserves `CutContour` and `Registrasi` groups untouched.
 */
export function applyClientRecolor(svg: string, spec: RecolorSpec): string {
	if (!svg || spec.mode === "original") return svg;
	const protectedIds = new Set(["CutContour", "Registrasi"]);
	try {
		const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
		const root = doc.documentElement;
		if (!root || root.tagName.toLowerCase() !== "svg") return svg;

		const walk = (el: Element) => {
			const id = el.getAttribute("id") || "";
			const layer = el.getAttribute("data-trasea-layer") || "";
			if (protectedIds.has(id) || protectedIds.has(layer)) return;

			for (const attr of ["fill", "stroke"]) {
				const v = el.getAttribute(attr);
				if (v && v.startsWith("#")) {
					const mapped = mapColor(v, spec);
					if (mapped) el.setAttribute(attr, mapped);
				}
			}
			const style = el.getAttribute("style");
			if (style && style.includes("#")) {
				el.setAttribute(
					"style",
					style.replace(/#[0-9a-fA-F]{6}\b/g, m => mapColor(m, spec) ?? m),
				);
			}
			for (let i = 0; i < el.children.length; i++) {
				const child = el.children[i];
				if (child) walk(child);
			}
		};
		walk(root);
		return new XMLSerializer().serializeToString(root);
	} catch {
		return svg;
	}
}
