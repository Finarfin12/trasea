import { useEffect, useRef, useState } from "react";
import { exportStockCsv, generateStockMetadata, getStockMetadataOptions, pickStockCsvPath } from "./api";
import { useI18n } from "./i18n";

const categoryNamesId = [
	"Hewan", "Bangunan dan Arsitektur", "Bisnis", "Minuman", "Lingkungan", "Keadaan Emosional",
	"Makanan", "Sumber Daya Grafis", "Hobi dan Rekreasi", "Industri", "Lanskap", "Gaya Hidup",
	"Orang", "Tanaman dan Bunga", "Budaya dan Agama", "Sains", "Isu Sosial", "Olahraga",
	"Teknologi", "Transportasi", "Perjalanan",
];
const providerNames: Record<string, string> = {
	claude: "Claude API", openai: "OpenAI API", gemini: "Gemini API",
	"claude-cli": "Claude CLI", "codex-cli": "Codex CLI",
	"gemini-cli": "Gemini CLI", "opencode-cli": "OpenCode CLI",
};

interface Props {
	open: boolean;
	suggestedFilename: string;
	onClose: () => void;
	onSettings: () => void;
}

export default function StockMetadataModal({ open, suggestedFilename, onClose, onSettings }: Props) {
	const { t, lang } = useI18n();
	const dialog = useRef<HTMLDialogElement>(null);
	const pending = useRef(false);
	const [options, setOptions] = useState<Awaited<ReturnType<typeof getStockMetadataOptions>>>();
	const [filenameOverride, setFilenameOverride] = useState<string>();
	const [title, setTitle] = useState("");
	const [keywordText, setKeywordText] = useState("");
	const [categoryId, setCategoryId] = useState(8);
	const [working, setWorking] = useState(false);
	const [status, setStatus] = useState("");
	const [error, setError] = useState("");
	const filename = filenameOverride ?? suggestedFilename;
	const titleLength = Array.from(title).length;
	const keywords = [...new Set(keywordText.split(/[,\n]/)
		.map(term => term.trim().replace(/\s+/g, " ").toLowerCase())
		.filter(Boolean))];

	useEffect(() => {
		const node = dialog.current;
		if (!node || !open) return;
		node.showModal();
		return () => node.close();
	}, [open]);

	useEffect(() => {
		if (!open) return;
		let active = true;
		setOptions(undefined);
		void getStockMetadataOptions()
			.then(next => { if (active) setOptions(next); })
			.catch(() => { if (active) setError(t.stockOptionsError); });
		return () => { active = false; };
	}, [open, t.stockOptionsError]);

	function messageFor(reason: unknown, fallback: string): string {
		const messages: Record<string, string> = {
			metadata_provider_unavailable: t.stockNoProvider,
			metadata_provider_changed: t.stockProviderChanged,
			metadata_response_invalid: t.stockBadResponse,
			metadata_preview_failed: t.stockPreviewError,
			metadata_csv_filename: t.stockCsvFilenameError,
			metadata_csv_title: t.stockCsvTitleError,
			metadata_csv_keywords: t.stockCsvKeywordsError,
		};
		return messages[String(reason)] ?? fallback;
	}

	async function generate() {
		const provider = options?.provider;
		if (!provider || pending.current) return;
		pending.current = true;
		setWorking(true);
		setError("");
		setStatus("");
		try {
			const draft = await generateStockMetadata(filename, provider);
			setTitle(draft.title);
			setKeywordText(draft.keywords.join(", "));
			setCategoryId(draft.categoryId);
			setStatus(t.stockGenerated);
		} catch (reason) {
			setError(messageFor(reason, t.stockGenerationError));
		} finally {
			pending.current = false;
			setWorking(false);
		}
	}

	async function copy(value: string) {
		setError("");
		setStatus("");
		try {
			await navigator.clipboard.writeText(value);
			setStatus(t.stockCopied);
		} catch {
			setError(t.stockCopyError);
		}
	}

	async function saveCsv() {
		if (pending.current) return;
		setError("");
		setStatus("");
		if (title.trim().length === 0 || Array.from(title.trim()).length > 70 || /[",]/.test(title)) {
			setError(t.stockCsvTitleError);
			return;
		}
		const asset = filename.trim();
		if (Array.from(asset).length > 30 || !/^[^/\\]+\.[^./\\]+$/.test(asset)) {
			setError(t.stockCsvFilenameError);
			return;
		}
		if (keywords.length === 0 || keywords.length > 49) {
			setError(t.stockCsvKeywordsError);
			return;
		}
		pending.current = true;
		setWorking(true);
		try {
			const outPath = await pickStockCsvPath(asset.replace(/\.[^.]+$/, "") + "-metadata.csv");
			if (outPath) {
				await exportStockCsv([{ filename: asset, title, keywords, categoryId, releases: "" }], outPath);
				setStatus(t.stockSaved);
			}
		} catch (reason) {
			setError(messageFor(reason, t.stockCsvError));
		} finally {
			pending.current = false;
			setWorking(false);
		}
	}

	return (
		<dialog className="stock-dialog modal-card" ref={dialog} aria-labelledby="stock-heading"
			onCancel={event => { event.preventDefault(); onClose(); }}
			onKeyDown={event => event.stopPropagation()}>
			<header className="modal-head">
				<h3 id="stock-heading">{t.stockMetadata}</h3>
				<button type="button" aria-label={t.stockClose} onClick={onClose}>×</button>
			</header>
			<div className="stock-ai">
				<div>
					{options?.provider ? <strong>{providerNames[options.provider]}</strong> : null}
					<p>{options && !options.provider ? t.stockNoProvider : t.stockPreviewNotice}</p>
				</div>
				<div className="stock-ai-actions">
					<button type="button" disabled={!options?.provider || working} onClick={() => void generate()}>{t.stockGenerate}</button>
					{options && !options.provider && <button type="button" onClick={onSettings}>{t.stockOpenSettings}</button>}
				</div>
			</div>
			<fieldset className="stock-inputs" disabled={working}>
				<label htmlFor="stock-filename">{t.stockFilename}</label>
				<input id="stock-filename" value={filename} onChange={event => setFilenameOverride(event.target.value)}
					aria-describedby="stock-filename-hint" />
				<small id="stock-filename-hint">{t.stockFilenameHint}</small>

				<div className="stock-input-head">
					<label htmlFor="stock-title">{t.stockTitle}</label>
					<small>{titleLength}/200</small>
				</div>
				<div className="stock-copy-field">
					<input id="stock-title" value={title} onChange={event => setTitle(Array.from(event.target.value).slice(0, 200).join(""))} />
					<button type="button" disabled={!title.trim()} onClick={() => void copy(title.trim())}>{t.stockCopyTitle}</button>
				</div>

				<div className="stock-input-head">
					<label htmlFor="stock-keywords">{t.stockKeywords}</label>
					<small>{keywords.length}/49</small>
				</div>
				<div className="stock-copy-field">
					<textarea id="stock-keywords" rows={3} value={keywordText} aria-describedby="stock-keywords-hint"
						onChange={event => setKeywordText(event.target.value)} />
					<button type="button" disabled={!keywords.length || keywords.length > 49}
						onClick={() => void copy(keywords.join(", "))}>{t.stockCopyKeywords}</button>
				</div>
				<small id="stock-keywords-hint">{t.stockKeywordsHint}</small>

				<label htmlFor="stock-category">{t.stockCategory}</label>
				<select id="stock-category" value={categoryId} onChange={event => setCategoryId(Number(event.target.value))}>
					{(lang === "id" ? categoryNamesId : options?.categories ?? []).map((name, index) => (
						<option key={name} value={index + 1}>{index + 1} · {name}</option>
					))}
				</select>
			</fieldset>
			{error && <p className="stock-error" role="alert">{error}</p>}
			<footer className="modal-foot">
				<small role="status">{working ? t.stockWorking : status || t.stockReview}</small>
				<button type="button" className="primary" disabled={working} onClick={() => void saveCsv()}>{t.stockExportCsv}</button>
			</footer>
		</dialog>
	);
}
