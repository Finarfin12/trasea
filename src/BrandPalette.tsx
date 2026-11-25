import { useState } from "react";
import { importSwatchPalette, parseHexPalette } from "./api";
import { useI18n } from "./i18n";

export interface BrandPalettePreset {
	id: string;
	name: string;
	colors: string[];
}

const storageKey = "trasea_brand_palettes_v1";

export function readBrandPalettes(): BrandPalettePreset[] {
	try {
		const data: unknown = JSON.parse(localStorage.getItem(storageKey) ?? "[]");
		return Array.isArray(data) ? data.filter((preset): preset is BrandPalettePreset =>
			preset && typeof preset.id === "string" && typeof preset.name === "string"
			&& Array.isArray(preset.colors) && preset.colors.length >= 2 && preset.colors.length <= 16
			&& preset.colors.every((color: unknown) => typeof color === "string" && /^#[0-9a-f]{6}$/i.test(color))
			&& new Set(preset.colors.map((color: string) => color.toUpperCase())).size === preset.colors.length) : [];
	} catch {
		return [];
	}
}

export function BrandPalette({ colors, disabled, onLock, onUnlock }: {
	colors: string[] | null;
	disabled: boolean;
	onLock: (colors: string[]) => void;
	onUnlock: () => void;
}) {
	const { t } = useI18n();
	const [presets, setPresets] = useState(readBrandPalettes);
	const [selected, setSelected] = useState("");
	const [name, setName] = useState("");
	const [hex, setHex] = useState("");
	const [showHex, setShowHex] = useState(false);
	const [working, setWorking] = useState(false);
	const [error, setError] = useState("");

	function save(next: BrandPalettePreset[]) {
		try {
			localStorage.setItem(storageKey, JSON.stringify(next));
			setPresets(next);
			setError("");
			return true;
		} catch {
			setError(t.brandSaveFailed);
			return false;
		}
	}

	async function apply(load: () => Promise<string[] | null>) {
		setWorking(true);
		setError("");
		try {
			const next = await load();
			if (!next) return;
			if (next.length < 2) {
				setError(t.brandMinimum);
				return;
			}
			onLock(next);
			setSelected("");
			setShowHex(false);
		} catch (e) {
			setError(String(e));
		} finally {
			setWorking(false);
		}
	}

	return (
		<fieldset className="brand-palette mode-box" disabled={disabled || working}>
			<legend>{t.brandLock}</legend>
			<div className="row">
				<button className="small" aria-expanded={showHex} onClick={() => setShowHex(!showHex)}>{t.brandPaste}</button>
				<button className="small" onClick={() => void apply(() => importSwatchPalette(t.brandImport))}>{t.brandImport}</button>
			</div>
			{showHex && (
				<div className="brand-palette">
					<textarea aria-label={t.brandPaste} placeholder="#0095D9, #E6007E, #F2B705" value={hex} onChange={e => setHex(e.target.value)} />
					<button className="small primary" onClick={() => void apply(() => parseHexPalette(hex))}>{t.brandApply}</button>
				</div>
			)}
			{colors && colors.length >= 2 && (
				<>
					<div className="row">
						<strong>{t.brandLocked} ({colors.length})</strong>
						<button className="small" onClick={() => { onUnlock(); setSelected(""); }}>{t.brandUnlock}</button>
					</div>
					<div className="row brand-swatches">
						{colors.map((color, index) => <span key={index} title={color} style={{ backgroundColor: color }} />)}
					</div>
					<div className="row">
						<input aria-label={t.brandPresetName} placeholder={t.brandPresetName} maxLength={80} value={name} onChange={e => setName(e.target.value)} />
						<button className="small" disabled={!name.trim()} onClick={() => {
							const preset = { id: crypto.randomUUID(), name: name.trim(), colors };
							if (save([...presets, preset])) {
								setSelected(preset.id);
								setName("");
							}
						}}>{t.brandSave}</button>
					</div>
				</>
			)}
			{presets.length > 0 && (
				<div className="row">
					<select aria-label={t.brandPresets} value={selected} onChange={e => {
						const preset = presets.find(item => item.id === e.target.value);
						if (preset) onLock(preset.colors);
						setSelected(e.target.value);
					}}>
						<option value="">{t.brandPresets}</option>
						{presets.map(preset => <option value={preset.id} key={preset.id}>{preset.name}</option>)}
					</select>
					<button className="small" disabled={!selected} onClick={() => {
						if (save(presets.filter(preset => preset.id !== selected))) setSelected("");
					}}>{t.brandDelete}</button>
				</div>
			)}
			{error && <small role="alert">{error}</small>}
		</fieldset>
	);
}
