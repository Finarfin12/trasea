import { useMemo, useState } from "react";
import { activateLicense, deactivateLicense, type LicenseStatus } from "./api";

interface Section {
	title: string;
	paragraphs: string[];
	header: string[];
	rows: string[][];
	codeBlocks: string[];
}

const cells = (line: string): string[] => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map(c => c.trim());

function parse(md: string): { title: string; intro: string[]; sections: Section[] } {
	let title = "Lisensi pihak ketiga";
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
			setOkMsg(`Lisensi aktif untuk ${next.info?.name ?? "Pengguna"} (100% offline).`);
		} catch (e) {
			setErr(typeof e === "string" ? e : e instanceof Error ? e.message : "Kunci lisensi tidak sah.");
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
			setOkMsg("Lisensi dihapus dari perangkat ini.");
		} catch (e) {
			setErr(typeof e === "string" ? e : e instanceof Error ? e.message : "Gagal menghapus lisensi.");
		} finally {
			setBusy(false);
		}
	};

	return (
		<div className="lic-wrap">
			<div className="seg" role="tablist" aria-label="Bagian Privasi dan Lisensi" style={{ marginTop: 8 }}>
				<button role="tab" aria-selected={tab === "license"} aria-pressed={tab === "license"} onClick={() => setTab("license")}>Status Lisensi</button>
				<button role="tab" aria-selected={tab === "privacy"} aria-pressed={tab === "privacy"} onClick={() => setTab("privacy")}>Janji 100% Offline</button>
				<button role="tab" aria-selected={tab === "thirdParty"} aria-pressed={tab === "thirdParty"} onClick={() => setTab("thirdParty")}>Lisensi Pihak Ketiga</button>
			</div>

			{tab === "license" && (
				<div className="lic-panel">
					{promptHint && !license.active && <div className="warn">{promptHint}</div>}
					<div className="lic-status-card">
						<div className="row between">
							<strong>Status saat ini</strong>
							<span className={`badge ${license.active ? "badge-ok" : "badge-cancelled"}`}>
								{license.active ? "Pro Aktif (Offline)" : "Trial Ramah"}
							</span>
						</div>
						{license.active && license.info ? (
							<>
								<dl className="stats" style={{ marginTop: 10 }}>
									<dt>Pemilik lisensi</dt><dd>{license.info.name}</dd>
									<dt>Tipe</dt><dd>Pro — Bayar Sekali</dd>
									<dt>ID Lisensi</dt><dd><code>{license.info.id}</code></dd>
									<dt>Tanggal terbit</dt><dd>{license.info.issued}</dd>
								</dl>
								<button className="small danger" onClick={() => void doDeactivate()} disabled={busy}>Hapus lisensi dari perangkat ini</button>
							</>
						) : (
							<div className="hint" style={{ marginTop: 8 }}>
								Editor tunggal dan ekspor SVG, PDF, serta palet .ase bebas dicoba tanpa watermark. Masukkan kunci lisensi offline <code>TRSA1-…</code> untuk membuka <b>Batch Folder</b>, <b>Ekspor EPS</b>, dan <b>Film Separasi Sablon</b>.
							</div>
						)}
					</div>

					{!license.active && (
						<div className="lic-form">
							<label htmlFor="licenseKeyInput">Masukkan kunci lisensi offline</label>
							<textarea
								id="licenseKeyInput"
								className="lic-input"
								rows={3}
								placeholder="TRSA1-..."
								value={keyInput}
								onChange={e => setKeyInput(e.target.value)}
							/>
							<button className="primary" onClick={() => void doActivate()} disabled={busy || !keyInput.trim()}>
								{busy ? "Memverifikasi…" : "Aktifkan lisensi (Offline)"}
							</button>
						</div>
					)}
					{err && <div className="error" role="alert">{err}</div>}
					{okMsg && <div className="ok" role="status">{okMsg}</div>}
				</div>
			)}

			{tab === "privacy" && (
				<div className="lic-panel">
					<h3>Janji 100% Offline & Privat</h3>
					<ul className="privacy-list">
						<li><b>Tidak ada unggah gambar:</b> seluruh proses pembersihan (Median / Real-ESRGAN), vektorisasi, dan ekspor berjalan lokal di komputer ini.</li>
						<li><b>Tidak ada telemetri atau pelacakan:</b> Trasea tidak mengumpulkan analitik penggunaan, crash report, atau data pribadi apa pun.</li>
						<li><b>Aktivasi lisensi 100% offline:</b> kunci <code>TRSA1-…</code> diverifikasi secara kriptografis di perangkat menggunakan Ed25519 tanpa pernah menghubungi server lisensi.</li>
						<li><b>Isolasi jaringan ketat:</b> crate inti <code>trasea-core</code> tidak memiliki dependensi HTTP/jaringan apa pun (diverifikasi otomatis oleh tes <code>V7</code> & <code>N-01</code>), dan WebView aplikasi membatasi <code>connect-src</code> hanya ke IPC lokal.</li>
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
								<summary>{sec.title} <span className="muted">({sec.rows.length > 0 ? `${sec.rows.length} baris` : "teks penuh"}, klik untuk membuka)</span></summary>
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
