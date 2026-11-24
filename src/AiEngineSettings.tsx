import { useCallback, useEffect, useState } from "react";
import {
	fetchAiModels,
	fetchCliModels,
	getAiConfig,
	saveAiConfig,
	testAiConfig,
	type AiConfigPublic,
	type AiModelItem,
	type AiProvider,
	type SaveAiConfigInput,
} from "./api";
import { useI18n } from "./i18n";

const EXTERNAL_PROVIDERS: AiProvider[] = ["recraft", "vectorizer_ai", "claude", "openai", "gemini", "cli"];
const MODEL_PROVIDERS: AiProvider[] = ["claude", "openai", "gemini"];

const CLI_PRESETS = {
	python: {
		executable: "python",
		args: ["vectorize.py", "{input}", "{output}"],
	},
	claude: {
		executable: "claude",
		args: [
			"-p",
			"Convert the image at {input} into a clean standalone SVG vector graphic with flat hex fills using a clean limited flat color palette (<= 10 hex colors, no near-duplicate shades) and return only <svg>...</svg>.",
		],
	},
	gemini: {
		executable: "gemini",
		args: [
			"-p",
			"Convert the image at {input} into a clean standalone SVG vector graphic with flat hex fills using a clean limited flat color palette (<= 10 hex colors, no near-duplicate shades) and return only <svg>...</svg>.",
		],
	},
	codex: {
		executable: "codex",
		args: [
			"exec",
			"--skip-git-repo-check",
			"--ephemeral",
			"--sandbox",
			"read-only",
			"-i",
			"{input}",
			"-o",
			"{output}",
			"Convert the attached image into a clean standalone SVG vector graphic with flat hex fills using a clean limited flat color palette (<= 10 hex colors, no near-duplicate shades) and return only <svg>...</svg>.",
		],
	},
	opencode: {
		executable: "opencode",
		args: [
			"run",
			"--auto",
			"-f",
			"{input}",
			"Convert the attached image into a clean standalone SVG vector graphic with flat hex fills using a clean limited flat color palette (<= 10 hex colors, no near-duplicate shades) and return only <svg>...</svg>.",
		],
	},
};

function cliKind(exe: string): "codex" | "claude" | "gemini" | "opencode" | "other" {
	const base = exe.trim().split(/[\\/]/).pop()?.toLowerCase().replace(/\.(exe|cmd|bat)$/, "") ?? "";
	if (base === "codex" || base === "claude" || base === "gemini" || base === "opencode") return base;
	return "other";
}

interface AiEngineSettingsProps {
	initialProvider?: AiProvider;
}

export default function AiEngineSettings({ initialProvider }: AiEngineSettingsProps = {}) {
	const { t } = useI18n();
	const [cfg, setCfg] = useState<AiConfigPublic | null>(null);
	const [selected, setSelected] = useState<AiProvider>(() =>
		initialProvider && EXTERNAL_PROVIDERS.includes(initialProvider) ? initialProvider : "recraft",
	);

	// Key input drafts (never pre-filled with secrets)
	const [recraftKey, setRecraftKey] = useState("");
	const [vecId, setVecId] = useState("");
	const [vecSecret, setVecSecret] = useState("");
	const [claudeKey, setClaudeKey] = useState("");
	const [openaiKey, setOpenaiKey] = useState("");
	const [geminiKey, setGeminiKey] = useState("");

	// CLI drafts
	const [cliExe, setCliExe] = useState("");
	const [cliArgsText, setCliArgsText] = useState("");
	const [cliTimeout, setCliTimeout] = useState(120);
	const [cliModel, setCliModel] = useState("");
	const [cliCustomModelMode, setCliCustomModelMode] = useState(false);
	const [cliReasoning, setCliReasoning] = useState("");
	const [cliModels, setCliModels] = useState<AiModelItem[]>([]);
	const [cliModelStatus, setCliModelStatus] = useState("");
	const [loadingCliModels, setLoadingCliModels] = useState(false);

	const loadCliModelsFor = useCallback(async (exe: string) => {
		const kind = cliKind(exe);
		if (kind === "other") {
			setCliModels([]);
			setCliModelStatus("");
			return;
		}
		setLoadingCliModels(true);
		try {
			const res = await fetchCliModels(exe);
			setCliModels(res.models);
			setCliModelStatus(res.message);
		} catch (e) {
			setCliModels([]);
			setCliModelStatus(typeof e === "string" ? e : String(e));
		} finally {
			setLoadingCliModels(false);
		}
	}, []);

	// Real dynamic model list (0 hardcoded models)
	const [models, setModels] = useState<AiModelItem[]>([]);
	const [modelStatus, setModelStatus] = useState("");
	const [loadingModels, setLoadingModels] = useState(false);

	// Feedback banners
	const [saving, setSaving] = useState(false);
	const [testing, setTesting] = useState(false);
	const [feedback, setFeedback] = useState<{ ok: boolean; text: string } | null>(null);

	const loadModelsFor = useCallback(
		async (provider: AiProvider, currentCfg?: AiConfigPublic | null) => {
			if (!MODEL_PROVIDERS.includes(provider)) {
				setModels([]);
				setModelStatus("");
				return;
			}
			setLoadingModels(true);
			try {
				const res = await fetchAiModels(provider);
				setModels(res.models);
				setModelStatus(res.message);
				if (res.models.length > 0 && currentCfg) {
					const savedModel =
						provider === "claude"
							? currentCfg.aiClaudeModel
							: provider === "openai"
								? currentCfg.aiOpenaiModel
								: currentCfg.aiGeminiModel;
					const stillValid = res.models.some(m => m.id === savedModel);
					if (!stillValid) {
						const firstId = res.models[0]?.id ?? "";
						if (firstId) {
							const patch: SaveAiConfigInput =
								provider === "claude"
									? { aiClaudeModel: firstId }
									: provider === "openai"
										? { aiOpenaiModel: firstId }
										: { aiGeminiModel: firstId };
							const updated = await saveAiConfig(patch);
							setCfg(updated);
						}
					}
				}
			} catch (e) {
				setModels([]);
				setModelStatus(typeof e === "string" ? e : String(e));
			} finally {
				setLoadingModels(false);
			}
		},
		[],
	);

	useEffect(() => {
		let alive = true;
		void getAiConfig()
			.then(loaded => {
				if (!alive) return;
				setCfg(loaded);
				setCliExe(loaded.aiCliExecutable);
				setCliArgsText(loaded.aiCliArgs.join("\n"));
				setCliTimeout(loaded.aiCliTimeoutSec || 120);
				setCliModel(loaded.aiCliModel ?? "");
				setCliReasoning(loaded.aiCliReasoningEffort ?? "");
				if (loaded.aiCliExecutable) {
					void loadCliModelsFor(loaded.aiCliExecutable);
				}
				const initial: AiProvider =
					initialProvider && EXTERNAL_PROVIDERS.includes(initialProvider)
						? initialProvider
						: EXTERNAL_PROVIDERS.includes(loaded.aiProvider as AiProvider)
							? (loaded.aiProvider as AiProvider)
							: "recraft";
				setSelected(initial);
				if (MODEL_PROVIDERS.includes(initial)) {
					void loadModelsFor(initial, loaded);
				}
			})
			.catch(() => {});
		return () => {
			alive = false;
		};
	}, [initialProvider, loadModelsFor, loadCliModelsFor]);

	const handleSelectProvider = (prov: AiProvider) => {
		setSelected(prov);
		setFeedback(null);
		if (MODEL_PROVIDERS.includes(prov)) {
			void loadModelsFor(prov, cfg);
		} else {
			setModels([]);
			setModelStatus("");
		}
	};

	const savePatch = async (patch: SaveAiConfigInput, reloadModelsProvider?: AiProvider) => {
		setSaving(true);
		setFeedback(null);
		try {
			const updated = await saveAiConfig({ aiProvider: selected, ...patch });
			setCfg(updated);
			setFeedback({ ok: true, text: t.aiConfigSaved });
			if (reloadModelsProvider && MODEL_PROVIDERS.includes(reloadModelsProvider)) {
				await loadModelsFor(reloadModelsProvider, updated);
			}
		} catch (e) {
			setFeedback({ ok: false, text: typeof e === "string" ? e : String(e) });
		} finally {
			setSaving(false);
		}
	};

	const handleTest = async () => {
		setTesting(true);
		setFeedback(null);
		try {
			const res = await testAiConfig(selected);
			setFeedback({ ok: res.ok, text: res.message });
			if (MODEL_PROVIDERS.includes(selected) && res.ok) {
				await loadModelsFor(selected, cfg);
			}
		} catch (e) {
			setFeedback({ ok: false, text: typeof e === "string" ? e : String(e) });
		} finally {
			setTesting(false);
		}
	};

	const providerLabel = (p: AiProvider): string => {
		switch (p) {
			case "recraft":
				return "Recraft V3";
			case "vectorizer_ai":
				return "Vectorizer.AI";
			case "claude":
				return "Claude";
			case "openai":
				return "GPT (OpenAI)";
			case "gemini":
				return "Gemini";
			case "cli":
				return t.aiProviderCli;
			default:
				return t.aiProviderLocal;
		}
	};

	const currentModelValue =
		selected === "claude"
			? cfg?.aiClaudeModel ?? ""
			: selected === "openai"
				? cfg?.aiOpenaiModel ?? ""
				: selected === "gemini"
					? cfg?.aiGeminiModel ?? ""
					: "";

	const hasKeyForSelected =
		selected === "recraft"
			? Boolean(cfg?.hasRecraftKey)
			: selected === "vectorizer_ai"
				? Boolean(cfg?.hasVectorizerKeys)
				: selected === "claude"
					? Boolean(cfg?.hasClaudeKey)
					: selected === "openai"
						? Boolean(cfg?.hasOpenaiKey)
						: selected === "gemini"
							? Boolean(cfg?.hasGeminiKey)
							: false;

	return (
		<div className="settings-section">
			<div className="hint" style={{ marginBottom: 10 }}>
				{t.aiGuidanceNote}
			</div>

			<div className="setting-row">
				<label htmlFor="ai-provider-select">{t.aiProviderLabel}</label>
				<select
					id="ai-provider-select"
					value={selected}
					onChange={e => handleSelectProvider(e.target.value as AiProvider)}
				>
					{EXTERNAL_PROVIDERS.map(p => (
						<option key={p} value={p}>
							{providerLabel(p)}
						</option>
					))}
				</select>
			</div>

			{selected === "recraft" && (
				<div className="update-card">
					<div className="row" style={{ justifyContent: "space-between", marginBottom: 6 }}>
						<strong>Recraft V3 API Key</strong>
						{hasKeyForSelected && <span className="pill ok">{t.aiKeySavedBadge}</span>}
					</div>
					<div className="row" style={{ gap: 6 }}>
						<input
							type="password"
							value={recraftKey}
							onChange={e => setRecraftKey(e.target.value)}
							placeholder={hasKeyForSelected ? "••••••••••••••••" : t.aiApiKeyPlaceholder}
							style={{ flex: 1 }}
						/>
						<button
							className="small primary"
							disabled={saving || !recraftKey.trim()}
							onClick={() => {
								const v = recraftKey.trim();
								setRecraftKey("");
								void savePatch({ recraftApiKey: v });
							}}
						>
							{t.aiSaveBtn}
						</button>
						{hasKeyForSelected && (
							<button
								className="small ghost"
								disabled={saving}
								onClick={() => void savePatch({ recraftApiKey: "" })}
							>
								{t.aiClearKeyBtn}
							</button>
						)}
					</div>
				</div>
			)}

			{selected === "vectorizer_ai" && (
				<div className="update-card">
					<div className="row" style={{ justifyContent: "space-between", marginBottom: 6 }}>
						<strong>Vectorizer.AI Credentials</strong>
						{hasKeyForSelected && <span className="pill ok">{t.aiKeySavedBadge}</span>}
					</div>
					<div style={{ display: "grid", gap: 6 }}>
						<input
							type="password"
							value={vecId}
							onChange={e => setVecId(e.target.value)}
							placeholder={hasKeyForSelected ? "API ID ••••••••" : "API ID"}
						/>
						<input
							type="password"
							value={vecSecret}
							onChange={e => setVecSecret(e.target.value)}
							placeholder={hasKeyForSelected ? "API Secret ••••••••" : "API Secret"}
						/>
						<div className="row" style={{ gap: 6 }}>
							<button
								className="small primary"
								disabled={saving || !vecId.trim() || !vecSecret.trim()}
								onClick={() => {
									const id = vecId.trim();
									const sec = vecSecret.trim();
									setVecId("");
									setVecSecret("");
									void savePatch({ vectorizerApiId: id, vectorizerApiSecret: sec });
								}}
							>
								{t.aiSaveBtn}
							</button>
							{hasKeyForSelected && (
								<button
									className="small ghost"
									disabled={saving}
									onClick={() =>
										void savePatch({ vectorizerApiId: "", vectorizerApiSecret: "" })
									}
								>
									{t.aiClearKeyBtn}
								</button>
							)}
						</div>
					</div>
				</div>
			)}

			{MODEL_PROVIDERS.includes(selected) && (
				<div className="update-card">
					<div className="row" style={{ justifyContent: "space-between", marginBottom: 6 }}>
						<strong>{providerLabel(selected)} API Key</strong>
						{hasKeyForSelected && <span className="pill ok">{t.aiKeySavedBadge}</span>}
					</div>
					<div className="row" style={{ gap: 6, marginBottom: 10 }}>
						<input
							type="password"
							value={selected === "claude" ? claudeKey : selected === "openai" ? openaiKey : geminiKey}
							onChange={e => {
								if (selected === "claude") setClaudeKey(e.target.value);
								else if (selected === "openai") setOpenaiKey(e.target.value);
								else setGeminiKey(e.target.value);
							}}
							placeholder={hasKeyForSelected ? "••••••••••••••••" : t.aiApiKeyPlaceholder}
							style={{ flex: 1 }}
						/>
						<button
							className="small primary"
							disabled={
								saving ||
								!(
									selected === "claude"
										? claudeKey
										: selected === "openai"
											? openaiKey
											: geminiKey
								).trim()
							}
							onClick={() => {
								if (selected === "claude") {
									const v = claudeKey.trim();
									setClaudeKey("");
									void savePatch({ claudeApiKey: v }, "claude");
								} else if (selected === "openai") {
									const v = openaiKey.trim();
									setOpenaiKey("");
									void savePatch({ openaiApiKey: v }, "openai");
								} else {
									const v = geminiKey.trim();
									setGeminiKey("");
									void savePatch({ geminiApiKey: v }, "gemini");
								}
							}}
						>
							{t.aiSaveBtn}
						</button>
						{hasKeyForSelected && (
							<button
								className="small ghost"
								disabled={saving}
								onClick={() => {
									if (selected === "claude") void savePatch({ claudeApiKey: "" }, "claude");
									else if (selected === "openai") void savePatch({ openaiApiKey: "" }, "openai");
									else void savePatch({ geminiApiKey: "" }, "gemini");
								}}
							>
								{t.aiClearKeyBtn}
							</button>
						)}
					</div>

					<div className="setting-row" style={{ marginBottom: 4 }}>
						<label htmlFor="ai-model-select">{t.aiModelLabel}</label>
						<div className="row" style={{ gap: 6, flex: 1 }}>
							<select
								id="ai-model-select"
								value={currentModelValue}
								disabled={loadingModels || models.length === 0}
								style={{ flex: 1 }}
								onChange={e => {
									const id = e.target.value;
									const patch: SaveAiConfigInput =
										selected === "claude"
											? { aiClaudeModel: id }
											: selected === "openai"
												? { aiOpenaiModel: id }
												: { aiGeminiModel: id };
									void savePatch(patch);
								}}
							>
								{models.length === 0 ? (
									<option value="">{t.aiNoModelsDetected}</option>
								) : (
									models.map(m => (
										<option key={m.id} value={m.id}>
											{m.label}
										</option>
									))
								)}
							</select>
							<button
								className="small"
								disabled={loadingModels}
								onClick={() => void loadModelsFor(selected, cfg)}
							>
								{loadingModels ? t.aiLoadingModels : t.aiReloadModelsBtn}
							</button>
						</div>
					</div>
					{modelStatus && <div className="hint">{modelStatus}</div>}
				</div>
			)}

			{selected === "cli" && (
				<div className="update-card">
					<div className="row" style={{ gap: 6, flexWrap: "wrap", marginBottom: 8 }}>
						<button
							className="small"
							onClick={() => {
								if (cliKind(cliExe) !== cliKind(CLI_PRESETS.python.executable)) {
									setCliModel("");
								}
								setCliCustomModelMode(false);
								setCliExe(CLI_PRESETS.python.executable);
								setCliArgsText(CLI_PRESETS.python.args.join("\n"));
								void loadCliModelsFor(CLI_PRESETS.python.executable);
							}}
						>
							{t.aiCliPresetPython}
						</button>
						<button
							className="small"
							onClick={() => {
								if (cliKind(cliExe) !== cliKind(CLI_PRESETS.claude.executable)) {
									setCliModel("");
								}
								setCliCustomModelMode(false);
								setCliExe(CLI_PRESETS.claude.executable);
								setCliArgsText(CLI_PRESETS.claude.args.join("\n"));
								void loadCliModelsFor(CLI_PRESETS.claude.executable);
							}}
						>
							Claude CLI
						</button>
						<button
							className="small"
							onClick={() => {
								if (cliKind(cliExe) !== cliKind(CLI_PRESETS.gemini.executable)) {
									setCliModel("");
								}
								setCliCustomModelMode(false);
								setCliExe(CLI_PRESETS.gemini.executable);
								setCliArgsText(CLI_PRESETS.gemini.args.join("\n"));
								void loadCliModelsFor(CLI_PRESETS.gemini.executable);
							}}
						>
							Gemini CLI
						</button>
						<button
							className="small"
							onClick={() => {
								if (cliKind(cliExe) !== cliKind(CLI_PRESETS.codex.executable)) {
									setCliModel("");
								}
								setCliCustomModelMode(false);
								setCliExe(CLI_PRESETS.codex.executable);
								setCliArgsText(CLI_PRESETS.codex.args.join("\n"));
								setCliReasoning("low");
								void loadCliModelsFor(CLI_PRESETS.codex.executable);
							}}
						>
							Codex CLI
						</button>
						<button
							className="small"
							onClick={() => {
								if (cliKind(cliExe) !== cliKind(CLI_PRESETS.opencode.executable)) {
									setCliModel("");
								}
								setCliCustomModelMode(false);
								setCliExe(CLI_PRESETS.opencode.executable);
								setCliArgsText(CLI_PRESETS.opencode.args.join("\n"));
								void loadCliModelsFor(CLI_PRESETS.opencode.executable);
							}}
						>
							OpenCode CLI
						</button>
					</div>

					<div className="setting-row">
						<label htmlFor="ai-cli-exe">{t.aiCliExecutableLabel}</label>
						<input
							id="ai-cli-exe"
							type="text"
							value={cliExe}
							onChange={e => setCliExe(e.target.value)}
							placeholder="python / claude / gemini / codex"
							style={{ flex: 1 }}
						/>
					</div>

					<div style={{ marginTop: 8 }}>
						<label htmlFor="ai-cli-args" style={{ display: "block", marginBottom: 4 }}>
							{t.aiCliArgsLabel}
						</label>
						<textarea
							id="ai-cli-args"
							rows={4}
							value={cliArgsText}
							onChange={e => setCliArgsText(e.target.value)}
							placeholder={"{input}\n{output}"}
							style={{ width: "100%", fontFamily: "monospace", fontSize: 12 }}
						/>
						<div className="hint">{t.aiCliArgsHint}</div>
					</div>

					{cliKind(cliExe) !== "other" && (() => {
						const isListedModel = cliModel !== "" && cliModels.some(m => m.id === cliModel);
						const showCustomInput = cliCustomModelMode || (cliModel !== "" && !isListedModel);
						const selectVal = showCustomInput ? "__custom__" : cliModel;
						return (
							<>
								<div className="setting-row" style={{ marginTop: 8 }}>
									<label htmlFor="ai-cli-model-select">{t.aiModelLabel}</label>
									<div className="row" style={{ gap: 6, flex: 1 }}>
										<select
											id="ai-cli-model-select"
											value={selectVal}
											onChange={e => {
												const v = e.target.value;
												if (v === "__custom__") {
													setCliCustomModelMode(true);
												} else {
													setCliCustomModelMode(false);
													setCliModel(v);
												}
											}}
											style={{ flex: 1 }}
										>
											<option value="">{t.aiCliModelDefaultOption}</option>
											{cliModels.map(m => (
												<option key={m.id} value={m.id}>
													{m.label}
												</option>
											))}
											<option value="__custom__">{t.aiCliModelCustomOption}</option>
										</select>
										<button
											className="small"
											disabled={loadingCliModels}
											onClick={() => void loadCliModelsFor(cliExe)}
										>
											{loadingCliModels ? t.aiLoadingModels : t.aiReloadModelsBtn}
										</button>
									</div>
								</div>
								{showCustomInput && (
									<div className="setting-row" style={{ marginTop: 6 }}>
										<label htmlFor="ai-cli-model-custom" />
										<input
											id="ai-cli-model-custom"
											type="text"
											value={cliModel}
											onChange={e => setCliModel(e.target.value)}
											placeholder={t.aiCliModelCustomPlaceholder}
											autoFocus
											style={{ flex: 1 }}
										/>
									</div>
								)}
								{cliModelStatus && <div className="hint">{cliModelStatus}</div>}

								{cliKind(cliExe) !== "gemini" && (
									<div className="setting-row" style={{ marginTop: 8 }}>
										<label htmlFor="ai-cli-reasoning">{t.aiCliReasoningLabel}</label>
										<select
											id="ai-cli-reasoning"
											value={cliReasoning}
											onChange={e => setCliReasoning(e.target.value)}
										>
											<option value="">{t.aiCliReasoningDefault}</option>
											<option value="low">{t.aiCliReasoningLow}</option>
											<option value="medium">{t.aiCliReasoningMedium}</option>
											<option value="high">{t.aiCliReasoningHigh}</option>
										</select>
									</div>
								)}
							</>
						);
					})()}

					<div className="setting-row" style={{ marginTop: 8 }}>
						<label htmlFor="ai-cli-timeout">{t.aiCliTimeoutLabel}</label>
						<input
							id="ai-cli-timeout"
							type="number"
							min={10}
							max={300}
							value={cliTimeout}
							onChange={e => setCliTimeout(Math.max(10, Math.min(300, Number(e.target.value) || 120)))}
							style={{ width: 90 }}
						/>
						<button
							className="small primary"
							disabled={saving}
							onClick={() => {
								const args = cliArgsText
									.split(/\r?\n/)
									.map(s => s.trim())
									.filter(Boolean);
								void savePatch({
									aiCliExecutable: cliExe.trim(),
									aiCliArgs: args,
									aiCliTimeoutSec: cliTimeout,
									aiCliModel: cliModel.trim(),
									aiCliReasoningEffort: cliReasoning,
								});
							}}
						>
							{t.aiSaveBtn}
						</button>
					</div>
				</div>
			)}

			<div className="row" style={{ gap: 8, marginTop: 8 }}>
				<button className="small" disabled={testing} onClick={() => void handleTest()}>
					{testing ? t.aiTestingBtn : t.aiTestConfigBtn}
				</button>
			</div>

			{feedback && (
				<div
					className={feedback.ok ? "ok" : "error"}
					role={feedback.ok ? "status" : "alert"}
					style={{ marginTop: 8 }}
				>
					{feedback.text}
				</div>
			)}
		</div>
	);
}
