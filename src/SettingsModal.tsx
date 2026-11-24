import { useEffect, useMemo, useState } from "react";
import Licenses from "./Licenses";
import {
	DEFAULT_APP_SETTINGS,
	DEFAULT_SHORTCUTS,
	checkForUpdate,
	getAppMode,
	openExternalUrl,
	licenses as fetchLicenses,
	type AppModeInfo,
	type AppSettings,
	type UpdateInfo,
	type CanvasBgChoice,
	type ColorSpace,
	type DxfUnit,
	type ShortcutAction,
	type ThemeChoice,
} from "./api";
import { effectiveShortcuts, eventToShortcutString } from "./colorUtils";
import { useI18n, type Lang } from "./i18n";

export type SettingsTab = "general" | "export" | "performance" | "shortcuts" | "privacy";

interface Props {
	settings: AppSettings;
	initialTab?: SettingsTab;
	licenseSubTab?: "privacy" | "thirdParty";
	onUpdate: (next: AppSettings) => void;
	onApplyExportDefaultsNow: (next: AppSettings) => void;
	onClose: () => void;
}

const SHORTCUT_ORDER: ShortcutAction[] = [
	"openFile",
	"paste",
	"copySvg",
	"saveProject",
	"undo",
	"redo",
	"toggleOutline",
	"togglePathSelect",
	"nodeTool",
	"toggleSoftProof",
	"openSettings",
];

export default function SettingsModal({
	settings,
	initialTab = "general",
	licenseSubTab = "privacy",
	onUpdate,
	onApplyExportDefaultsNow,
	onClose,
}: Props) {
	const { t } = useI18n();
	const [tab, setTab] = useState<SettingsTab>(initialTab);
	const [appMode, setAppMode] = useState<AppModeInfo | null>(null);
	const [checkingUpdate, setCheckingUpdate] = useState(false);
	const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
	const [updateErr, setUpdateErr] = useState("");
	const [licensesMd, setLicensesMd] = useState("");
	const [recording, setRecording] = useState<ShortcutAction | null>(null);
	const [msg, setMsg] = useState("");

	useEffect(() => {
		void getAppMode().then(setAppMode).catch(() => {});
		void fetchLicenses().then(setLicensesMd).catch(() => {});
	}, []);

	const activeShortcuts = useMemo(() => effectiveShortcuts(settings.shortcuts), [settings.shortcuts]);

	const actionLabels: Record<ShortcutAction, string> = useMemo(
		() => ({
			openFile: t.actOpenFile,
			paste: t.actPaste,
			copySvg: t.actCopySvg,
			saveProject: t.actSaveProject,
			undo: t.actUndo,
			redo: t.actRedo,
			toggleOutline: t.actToggleOutline,
			togglePathSelect: t.actTogglePathSelect,
			nodeTool: t.actNodeTool,
			toggleSoftProof: t.actToggleSoftProof,
			openSettings: t.actOpenSettings,
		}),
		[t],
	);

	// Detect duplicate shortcut combos across actions
	const conflicts = useMemo(() => {
		const byCombo = new Map<string, ShortcutAction[]>();
		for (const act of SHORTCUT_ORDER) {
			const norm = activeShortcuts[act].toLowerCase();
			const list = byCombo.get(norm) ?? [];
			list.push(act);
			byCombo.set(norm, list);
		}
		const map = new Map<ShortcutAction, ShortcutAction[]>();
		for (const list of byCombo.values()) {
			if (list.length > 1) {
				for (const act of list) {
					map.set(act, list.filter(x => x !== act));
				}
			}
		}
		return map;
	}, [activeShortcuts]);

	useEffect(() => {
		if (!recording) return;
		const onKey = (e: KeyboardEvent) => {
			e.preventDefault();
			e.stopPropagation();
			if (e.key === "Escape") {
				setRecording(null);
				return;
			}
			const combo = eventToShortcutString(e);
			if (!combo) return;
			const nextShortcuts = { ...settings.shortcuts, [recording]: combo };
			onUpdate({ ...settings, shortcuts: nextShortcuts });
			setRecording(null);
			setMsg(t.settingsSavedMsg);
		};
		window.addEventListener("keydown", onKey, true);
		return () => window.removeEventListener("keydown", onKey, true);
	}, [recording, settings, onUpdate, t.settingsSavedMsg]);

	const patch = (p: Partial<AppSettings>) => {
		onUpdate({ ...settings, ...p });
		setMsg(t.settingsSavedMsg);
	};

	const resetAll = () => {
		onUpdate({ ...DEFAULT_APP_SETTINGS, language: settings.language });
		setMsg(t.settingsSavedMsg);
	};

	const resetShortcuts = () => {
		onUpdate({ ...settings, shortcuts: {} });
		setRecording(null);
		setMsg(t.settingsSavedMsg);
	};

	const runCheckUpdate = async () => {
		setCheckingUpdate(true);
		setUpdateErr("");
		try {
			const info = await checkForUpdate();
			setUpdateInfo(info);
		} catch (e) {
			setUpdateInfo(null);
			setUpdateErr(String(e));
		} finally {
			setCheckingUpdate(false);
		}
	};
	return (
		<div className="modal" role="dialog" aria-modal="true" aria-label={t.settingsModalTitle} onClick={onClose}>
			<div className="modal-box settings-modal-box" onClick={e => e.stopPropagation()}>
				<div className="row between">
					<h2 style={{ margin: 0 }}>{t.settingsModalTitle}</h2>
					<button className="ghost small" onClick={onClose}>{t.close}</button>
				</div>

				<div className="seg seg5" role="tablist" aria-label={t.settingsModalTitle} style={{ marginTop: 10 }}>
					<button role="tab" aria-selected={tab === "general"} aria-pressed={tab === "general"} onClick={() => setTab("general")}>{t.tabGeneral}</button>
					<button role="tab" aria-selected={tab === "export"} aria-pressed={tab === "export"} onClick={() => setTab("export")}>{t.tabExportDefaults}</button>
					<button role="tab" aria-selected={tab === "performance"} aria-pressed={tab === "performance"} onClick={() => setTab("performance")}>{t.tabPerformance}</button>
					<button role="tab" aria-selected={tab === "shortcuts"} aria-pressed={tab === "shortcuts"} onClick={() => setTab("shortcuts")}>{t.tabShortcuts}</button>
					<button role="tab" aria-selected={tab === "privacy"} aria-pressed={tab === "privacy"} onClick={() => setTab("privacy")}>{t.privacyCredits}</button>
				</div>

				<div className="settings-body">
					{tab === "general" && (
						<div className="settings-section">
							<div className="setting-row">
								<label>{t.settingLanguage}</label>
								<div className="seg seg2" role="group" aria-label={t.settingLanguage}>
									<button aria-pressed={settings.language === "id"} onClick={() => patch({ language: "id" as Lang })}>Indonesia (ID)</button>
									<button aria-pressed={settings.language === "en"} onClick={() => patch({ language: "en" as Lang })}>English (EN)</button>
								</div>
							</div>

							<div className="setting-row">
								<label>{t.settingTheme}</label>
								<div className="seg" role="group" aria-label={t.settingTheme}>
									<button aria-pressed={settings.theme === "dark"} onClick={() => patch({ theme: "dark" as ThemeChoice })}>{t.themeDark}</button>
									<button aria-pressed={settings.theme === "light"} onClick={() => patch({ theme: "light" as ThemeChoice })}>{t.themeLight}</button>
									<button aria-pressed={settings.theme === "system"} onClick={() => patch({ theme: "system" as ThemeChoice })}>{t.themeSystem}</button>
								</div>
							</div>

							<div className="setting-row">
								<label htmlFor="settingCanvasBg">{t.settingCanvasBg}</label>
								<select
									id="settingCanvasBg"
									className="setting-select"
									value={settings.canvasBg}
									onChange={e => patch({ canvasBg: e.target.value as CanvasBgChoice })}
								>
									<option value="checkerDark">{t.canvasCheckerDark}</option>
									<option value="checkerLight">{t.canvasCheckerLight}</option>
									<option value="solid">{t.canvasSolid}</option>
								</select>
							</div>

							<label className="check inline" style={{ marginTop: 8 }}>
								<input
									type="checkbox"
									checked={settings.openFolderAfterExport}
									onChange={e => patch({ openFolderAfterExport: e.target.checked })}
								/>
								<span>{t.settingOpenFolderExport}</span>
							</label>

						</div>
					)}

					{tab === "export" && (
						<div className="settings-section">
							<div className="setting-row">
								<label>{t.settingDefaultColorSpace}</label>
								<div className="seg seg2" role="group" aria-label={t.settingDefaultColorSpace}>
									<button aria-pressed={settings.defaultColorSpace === "rgb"} onClick={() => patch({ defaultColorSpace: "rgb" as ColorSpace })}>RGB</button>
									<button aria-pressed={settings.defaultColorSpace === "cmyk"} onClick={() => patch({ defaultColorSpace: "cmyk" as ColorSpace })}>CMYK</button>
								</div>
							</div>

							<div className="setting-row">
								<label>{t.settingDefaultDxfUnit}</label>
								<div className="seg seg2" role="group" aria-label={t.settingDefaultDxfUnit}>
									<button aria-pressed={settings.defaultDxfUnit === "mm"} onClick={() => patch({ defaultDxfUnit: "mm" as DxfUnit })}>mm</button>
									<button aria-pressed={settings.defaultDxfUnit === "px"} onClick={() => patch({ defaultDxfUnit: "px" as DxfUnit })}>px</button>
								</div>
							</div>

							<div className="setting-row">
								<label>{t.settingDefaultDpi}</label>
								<div className="seg" role="group" aria-label={t.settingDefaultDpi}>
									<button aria-pressed={settings.defaultDpi === 72} onClick={() => patch({ defaultDpi: 72 })}>72 DPI</button>
									<button aria-pressed={settings.defaultDpi === 96} onClick={() => patch({ defaultDpi: 96 })}>96 DPI</button>
									<button aria-pressed={settings.defaultDpi === 300} onClick={() => patch({ defaultDpi: 300 })}>300 DPI</button>
								</div>
							</div>

							<div className="setting-row">
								<label htmlFor="defDieCutOffset">{t.settingDefaultDieCutOffset}: <b>{settings.defaultDieCutOffsetMm.toFixed(1)} mm</b></label>
								<input
									id="defDieCutOffset"
									type="range"
									min={0.5}
									max={10}
									step={0.5}
									value={settings.defaultDieCutOffsetMm}
									onChange={e => patch({ defaultDieCutOffsetMm: Number(e.target.value) })}
									style={{ accentColor: "var(--accent)" }}
								/>
							</div>

							<div className="setting-row">
								<label htmlFor="defDieCutSpot">{t.settingDefaultDieCutSpot}</label>
								<input
									id="defDieCutSpot"
									className="setting-input"
									type="text"
									value={settings.defaultDieCutSpotName}
									onChange={e => patch({ defaultDieCutSpotName: e.target.value })}
									placeholder="CutContour"
								/>
							</div>

							<label className="check inline" style={{ marginTop: 6 }}>
								<input
									type="checkbox"
									checked={settings.defaultDieCutBridge}
									onChange={e => patch({ defaultDieCutBridge: e.target.checked })}
								/>
								<span>{t.settingDefaultDieCutBridge}</span>
							</label>

							<div className="row" style={{ marginTop: 10 }}>
								<button className="small" onClick={() => { onApplyExportDefaultsNow(settings); setMsg(t.settingsSavedMsg); }}>
									{t.applyExportDefaultsNow}
								</button>
							</div>
						</div>
					)}

					{tab === "performance" && (
						<div className="settings-section">
							<div className="setting-row">
								<label>{t.settingCleanerMode}</label>
								<div className="seg seg2" role="group" aria-label={t.settingCleanerMode}>
									<button aria-pressed={!settings.preferMedianOnly} onClick={() => patch({ preferMedianOnly: false })}>{t.cleanerAutoGpu}</button>
									<button aria-pressed={settings.preferMedianOnly} onClick={() => patch({ preferMedianOnly: true })}>{t.cleanerMedianFast}</button>
								</div>
							</div>

							<div className="setting-row">
								<label>{t.settingUndoLimit}</label>
								<div className="seg" role="group" aria-label={t.settingUndoLimit}>
									<button aria-pressed={settings.undoLimit === 20} onClick={() => patch({ undoLimit: 20 })}>20 {t.undoStepsUnit}</button>
									<button aria-pressed={settings.undoLimit === 50} onClick={() => patch({ undoLimit: 50 })}>50 {t.undoStepsUnit}</button>
									<button aria-pressed={settings.undoLimit === 100} onClick={() => patch({ undoLimit: 100 })}>100 {t.undoStepsUnit}</button>
								</div>
							</div>

							<div className="row" style={{ marginTop: 14 }}>
								<button className="danger small" onClick={resetAll}>{t.resetAllSettings}</button>
							</div>
						</div>
					)}

					{tab === "shortcuts" && (
						<div className="settings-section">
							<div className="hint" style={{ marginBottom: 8 }}>{t.shortcutsHint}</div>
							<div className="shortcut-list">
								{SHORTCUT_ORDER.map(act => {
									const combo = activeShortcuts[act];
									const isRec = recording === act;
									const conflictList = conflicts.get(act);
									return (
										<div key={act} className="shortcut-item">
											<div className="shortcut-name">
												<span>{actionLabels[act]}</span>
												{conflictList && (
													<span className="shortcut-conflict">
														⚠ {t.shortcutConflictWarn} {conflictList.map(c => actionLabels[c]).join(", ")}
													</span>
												)}
											</div>
											<button
												className={`small shortcut-key ${isRec ? "recording" : ""}`}
												aria-pressed={isRec}
												onClick={() => setRecording(isRec ? null : act)}
											>
												{isRec ? t.shortcutListening : combo}
											</button>
										</div>
									);
								})}
							</div>
							<div className="row between" style={{ marginTop: 10 }}>
								<span className="hint">Default: {DEFAULT_SHORTCUTS.openSettings} ({t.actOpenSettings})</span>
								<button className="small" onClick={resetShortcuts}>{t.resetShortcuts}</button>
							</div>
						</div>
					)}

					{tab === "privacy" && (
						<div className="settings-section">
							<div className="update-card">
								<div className="row between">
									<div>
										<strong>{t.updateSectionTitle}</strong>
										<div className="muted" style={{ fontSize: 12, marginTop: 2 }}>
											{t.appModeLabel}: {appMode?.isPortable ? t.appModePortable : t.appModeInstalled}
											{appMode ? ` · v${appMode.currentVersion}` : ""}
										</div>
									</div>
									<button className="small" onClick={() => void runCheckUpdate()} disabled={checkingUpdate}>
										{checkingUpdate ? t.updateChecking : t.updateCheckBtn}
									</button>
								</div>
								<label className="check inline" style={{ marginTop: 8 }}>
									<input
										type="checkbox"
										checked={settings.autoCheckUpdate}
										onChange={e => patch({ autoCheckUpdate: e.target.checked })}
									/>
									<span>{t.updateAutoCheckLabel}</span>
								</label>
								{updateErr && (
									<div className="hint" role="status" style={{ marginTop: 8, color: "var(--warn, #f59e0b)" }}>
										{updateErr}
									</div>
								)}
								{updateInfo && !updateInfo.hasUpdate && (
									<div className="ok" role="status" style={{ marginTop: 8 }}>
										{t.updateUpToDate} (v{updateInfo.currentVersion})
									</div>
								)}
								{updateInfo && updateInfo.hasUpdate && (
									<div className="update-available" role="status" style={{ marginTop: 8 }}>
										<div className="row between">
											<strong>{t.updateAvailable}: v{updateInfo.latestVersion}</strong>
											<button className="small primary" onClick={() => void openExternalUrl(updateInfo.downloadUrl).catch(() => {})}>
												{t.updateDownloadBtn}
											</button>
										</div>
										{updateInfo.releaseNotes && (
											<pre className="update-notes">{updateInfo.releaseNotes}</pre>
										)}
									</div>
								)}
							</div>
							<Licenses
								markdown={licensesMd}
								initialTab={licenseSubTab}
							/>
						</div>
					)}
				</div>

				{msg && tab !== "privacy" && <div className="ok" role="status" style={{ marginTop: 10 }}>{msg}</div>}
			</div>
		</div>
	);
}
