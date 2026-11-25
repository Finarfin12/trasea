import type { Dict } from "./i18n/id";

export interface EngineSnapshot {
	svg: string;
	paths: number;
	nodes: number;
	bytes: number;
	label: string;
}

interface CompareEngineModalProps {
	offline: EngineSnapshot | null;
	ai: EngineSnapshot | null;
	activeSource: "offline" | "ai";
	busy: boolean;
	t: Record<keyof Dict, string>;
	onRunOffline: () => void;
	onRunAi: () => void;
	onPick: (source: "offline" | "ai") => void;
	onClose: () => void;
}

function fmtKb(bytes: number): string {
	return `${(bytes / 1024).toFixed(1)} KB`;
}

function svgDataUrl(svg: string): string {
	const cleaned = svg.includes("xmlns=")
		? svg
		: svg.replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg"');
	return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(cleaned)}`;
}

export function CompareEngineModal({
	offline,
	ai,
	activeSource,
	busy,
	t,
	onRunOffline,
	onRunAi,
	onPick,
	onClose,
}: CompareEngineModalProps) {
	return (
		<div className="modal-backdrop" role="dialog" aria-modal="true" aria-label={t.compareEnginesTitle}>
			<div className="modal-card compare-modal">
				<div className="modal-head">
					<h3>{t.compareEnginesTitle}</h3>
					<button className="small" onClick={onClose}>{t.close}</button>
				</div>

				<div className="compare-grid">
					{/* Left: Offline GPU */}
					<div className={`compare-col ${activeSource === "offline" ? "is-active" : ""}`}>
						<div className="compare-col-head">
							<span className="compare-badge">{t.compareOfflineTitle}</span>
							{offline && (
								<span className="compare-metrics tabular">
									{offline.paths} {t.statPaths.toLowerCase()} · {offline.nodes} {t.statNodes.toLowerCase()} · {fmtKb(offline.bytes)}
								</span>
							)}
						</div>

						<div className="compare-preview">
							{offline ? (
								<img src={svgDataUrl(offline.svg)} alt={t.compareOfflineTitle} />
							) : (
								<div className="compare-empty">
									<p>{t.compareNotRunYet}</p>
									<button className="primary" onClick={onRunOffline} disabled={busy}>
										{t.compareRunOffline}
									</button>
								</div>
							)}
						</div>

						<div className="compare-col-foot">
							<button
								className={activeSource === "offline" ? "primary" : ""}
								disabled={!offline || busy}
								onClick={() => onPick("offline")}
							>
								{t.compareUseOffline}
							</button>
						</div>
					</div>

					{/* Right: AI Engine */}
					<div className={`compare-col ${activeSource === "ai" ? "is-active" : ""}`}>
						<div className="compare-col-head">
							<span className="compare-badge ai">{t.compareAiTitle}</span>
							{ai && (
								<span className="compare-metrics tabular">
									{ai.paths} {t.statPaths.toLowerCase()} · {ai.nodes} {t.statNodes.toLowerCase()} · {fmtKb(ai.bytes)}
								</span>
							)}
						</div>

						<div className="compare-preview">
							{ai ? (
								<img src={svgDataUrl(ai.svg)} alt={t.compareAiTitle} />
							) : (
								<div className="compare-empty">
									<p>{t.compareNotRunYet}</p>
									<button className="primary" onClick={onRunAi} disabled={busy}>
										{t.compareRunAi}
									</button>
								</div>
							)}
						</div>

						<div className="compare-col-foot">
							<button
								className={activeSource === "ai" ? "primary" : ""}
								disabled={!ai || busy}
								onClick={() => onPick("ai")}
							>
								{t.compareUseAi}
							</button>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
