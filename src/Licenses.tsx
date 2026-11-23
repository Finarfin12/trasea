import { useMemo, useState } from "react";
import { activateLicense, deactivateLicense, type LicenseStatus } from "./api";
import { useI18n } from "./i18n";

interface Section {
	title: string;
	paragraphs: string[];
	header: string[];
	rows: string[][];
	codeBlocks: string[];
}

const cells = (line: string): string[] => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map(c => c.trim());

function parse(md: string): { title: string; intro: string[]; sections: Section[] } {
	let title = "Third-party licenses";
	const intro: string[] = [];
	const sections: Section[] = [];
	let cur: Section | null = null;
	let inFence = false;
	let fenceLines: string[] = [];
	for (const raw of md.split(/\r?\n/)) {
		const line = raw.trimEnd();
		if (line.startsWith("```")) {
			if (!inFence) {
				inFence = true;
				fenceLines = [];
			} else {
				inFence = false;
				if (cur) cur.codeBlocks.push(fenceLines.join("\n"));
			}
			continue;
		}
		if (inFence) {
			fenceLines.push(raw);
			continue;
		}
		if (line.startsWith("# ")) {
			title = line.slice(2);
		} else if (line.startsWith("## ")) {
			cur = { title: line.slice(3), paragraphs: [], header: [], rows: [], codeBlocks: [] };
			sections.push(cur);
		} else if (line.startsWith("|")) {
			if (!cur) continue;
			if (/^\|\s*-{2,}/.test(line) || /^\|[\s|:-]+$/.test(line)) continue;
			if (cur.header.length === 0) cur.header = cells(line);
			else cur.rows.push(cells(line));
		} else if (line.trim() !== "") {
			(cur ? cur.paragraphs : intro).push(line);
		}
	}
	return { title, intro, sections };
}

const isUrl = (s: string): boolean => /^https?:\/\/\S+$/.test(s);

function Table({ header, rows }: { header: string[]; rows: string[][] }) {
	return (
		<div className="lic-scroll">
			<table className="lic-table">
				<thead>
					<tr>{header.map(h => <th key={h}>{h}</th>)}</tr>
				</thead>
				<tbody>
					{rows.map((r, i) => (
						<tr key={i}>{r.map((c, j) => <td key={j}>{isUrl(c) ? <span className="lic-url">{c}</span> : c}</td>)}</tr>
					))}
				</tbody>
			</table>
		</div>
	);
}

interface Props {
	markdown: string;
	license: LicenseStatus;
	initialTab?: "license" | "privacy" | "thirdParty";
	promptHint?: string;
	onLicenseChanged: (next: LicenseStatus) => void;
}

export default function Licenses({ markdown, license, initialTab = "license", promptHint = "", onLicenseChanged }: Props) {
	const { t } = useI18n();
	const [tab, setTab] = useState<"license" | "privacy" | "thirdParty">(initialTab);
	const [keyInput, setKeyInput] = useState("");
	const [busy, setBusy] = useState(false);
	const [err, setErr] = useState("");
	const [okMsg, setOkMsg] = useState("");
	const doc = useMemo(() => parse(markdown), [markdown]);

	const doActivate = async () => {
		if (!keyInput.trim() || busy) return;
		setBusy(true);
		setErr("");
		setOkMsg("");
		try {
			const next = await activateLicense(keyInput.trim());
			onLicenseChanged(next);
			setKeyInput("");
			setOkMsg(`${t.licActivatedPrefix} ${next.info?.name ?? "User"} ${t.licLocalParen}.`);
		} catch (e) {
			setErr(typeof e === "string" ? e : e instanceof Error ? e.message : t.licInvalidErr);
		} finally {
			setBusy(false);
		}
	};

	const doDeactivate = async () => {
		if (busy) return;
		setBusy(true);
		setErr("");
		setOkMsg("");
		try {
			const next = await deactivateLicense();
			onLicenseChanged(next);
			setOkMsg(t.licRemovedMsg);
		} catch (e) {
			setErr(typeof e === "string" ? e : e instanceof Error ? e.message : t.licRemoveErr);
		} finally {
			setBusy(false);
		}
	};

	return (
		<div className="lic-wrap">
			<div className="seg" role="tablist" aria-label={t.privacyLicense} style={{ marginTop: 8 }}>
				<button role="tab" aria-selected={tab === "license"} aria-pressed={tab === "license"} onClick={() => setTab("license")}>{t.licTabStatus}</button>
				<button role="tab" aria-selected={tab === "privacy"} aria-pressed={tab === "privacy"} onClick={() => setTab("privacy")}>{t.licTabPrivacy}</button>
				<button role="tab" aria-selected={tab === "thirdParty"} aria-pressed={tab === "thirdParty"} onClick={() => setTab("thirdParty")}>{t.licTabThirdParty}</button>
			</div>

			{tab === "license" && (
				<div className="lic-panel">
					{promptHint && !license.active && <div className="warn">{promptHint}</div>}
					<div className="lic-status-card">
						<div className="row between">
							<strong>{t.licCurrentStatus}</strong>
							<span className={`badge ${license.active ? "badge-ok" : "badge-cancelled"}`}>
								{license.active ? t.licProActiveBadge : t.licTrialBadge}
							</span>
						</div>
						{license.active && license.info ? (
							<>
								<dl className="stats" style={{ marginTop: 10 }}>
									<dt>{t.licOwner}</dt><dd>{license.info.name}</dd>
									<dt>{t.licType}</dt><dd>{t.licTypeValue}</dd>
									<dt>{t.licId}</dt><dd><code>{license.info.id}</code></dd>
									<dt>{t.licIssued}</dt><dd>{license.info.issued}</dd>
								</dl>
								<button className="small danger" onClick={() => void doDeactivate()} disabled={busy}>{t.licRemoveBtn}</button>
							</>
						) : (
							<div className="hint" style={{ marginTop: 8 }}>{t.licTrialDesc}</div>
						)}
					</div>

					{!license.active && (
						<div className="lic-form">
							<label htmlFor="licenseKeyInput">{t.licInputLabel}</label>
							<textarea
								id="licenseKeyInput"
								className="lic-input"
								rows={3}
								placeholder="TRSA1-..."
								value={keyInput}
								onChange={e => setKeyInput(e.target.value)}
							/>
							<button className="primary" onClick={() => void doActivate()} disabled={busy || !keyInput.trim()}>
								{busy ? t.licVerifying : t.licActivateBtn}
							</button>
						</div>
					)}
					{err && <div className="error" role="alert">{err}</div>}
					{okMsg && <div className="ok" role="status">{okMsg}</div>}
				</div>
			)}

			{tab === "privacy" && (
				<div className="lic-panel">
					<h3>{t.privacyTitle}</h3>
					<ul className="privacy-list">
						<li><b>{t.privacyBullet1Title}</b> {t.privacyBullet1Body}</li>
						<li><b>{t.privacyBullet2Title}</b> {t.privacyBullet2Body}</li>
						<li><b>{t.privacyBullet3Title}</b> {t.privacyBullet3Body}</li>
						<li><b>{t.privacyBullet4Title}</b> {t.privacyBullet4Body}</li>
					</ul>
				</div>
			)}

			{tab === "thirdParty" && (
				<div className="lic">
					{doc.intro.map((p, i) => <p key={i}>{p}</p>)}
					{doc.sections.map(sec => {
						const long = sec.rows.length > 20 || sec.codeBlocks.length > 0;
						const body = (
							<>
								{sec.paragraphs.map((p, i) => <p key={i}>{p}</p>)}
								{sec.header.length > 0 && <Table header={sec.header} rows={sec.rows} />}
								{sec.codeBlocks.map((cb, i) => <pre key={i} className="lic-pre">{cb}</pre>)}
							</>
						);
						return long ? (
							<details key={sec.title} className="lic-details">
								<summary>{sec.title} <span className="muted">({sec.rows.length > 0 ? `${sec.rows.length} ${t.thirdPartyRowsUnit}` : t.thirdPartyFullText}, {t.thirdPartyClickOpen})</span></summary>
								{body}
							</details>
						) : (
							<section key={sec.title}>
								<h3>{sec.title}</h3>
								{body}
							</section>
						);
					})}
				</div>
			)}
		</div>
	);
}
