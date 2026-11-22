import { useMemo } from "react";

interface Section {
	title: string;
	paragraphs: string[];
	header: string[];
	rows: string[][];
}

const cells = (line: string): string[] => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map(c => c.trim());

/** Turns THIRD_PARTY_LICENSES.md (headings, paragraphs, pipe tables) into sections. */
function parse(md: string): { title: string; intro: string[]; sections: Section[] } {
	let title = "Lisensi pihak ketiga";
	const intro: string[] = [];
	const sections: Section[] = [];
	let cur: Section | null = null;
	for (const raw of md.split(/\r?\n/)) {
		const line = raw.trimEnd();
		if (line.startsWith("# ")) {
			title = line.slice(2);
		} else if (line.startsWith("## ")) {
			cur = { title: line.slice(3), paragraphs: [], header: [], rows: [] };
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

export default function Licenses({ markdown }: { markdown: string }) {
	const doc = useMemo(() => parse(markdown), [markdown]);
	return (
		<div className="lic">
			{doc.intro.map((p, i) => <p key={i}>{p}</p>)}
			{doc.sections.map(sec => {
				// the long crate list is collapsed so the programs and models stay in view
				const long = sec.rows.length > 20;
				const body = (
					<>
						{sec.paragraphs.map((p, i) => <p key={i}>{p}</p>)}
						{sec.header.length > 0 && <Table header={sec.header} rows={sec.rows} />}
					</>
				);
				return long ? (
					<details key={sec.title} className="lic-details">
						<summary>{sec.title} <span className="muted">({sec.rows.length} baris, klik untuk membuka)</span></summary>
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
	);
}
