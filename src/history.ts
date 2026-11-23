import type { BrushStroke, ExportOptions, Options } from "./api";

export type TargetChoice = "auto" | "500" | "1000" | "2000" | "custom";

export interface EditorSnapshot {
	opts: Options;
	palette: string[] | null;
	paletteMerges: Array<[string, string]>;
	strokes: BrushStroke[];
	targetChoice: TargetChoice;
	customTarget: string;
	exp: ExportOptions;
	label: string;
}

export interface HistoryState {
	past: EditorSnapshot[];
	future: EditorSnapshot[];
}

export const MAX_HISTORY = 50;

export const emptyHistory = (): HistoryState => ({ past: [], future: [] });

export function pushSnapshot(h: HistoryState, prev: EditorSnapshot): HistoryState {
	const nextPast = [...h.past, prev];
	if (nextPast.length > MAX_HISTORY) nextPast.splice(0, nextPast.length - MAX_HISTORY);
	return { past: nextPast, future: [] };
}

export function stepUndo(h: HistoryState, current: EditorSnapshot): { history: HistoryState; target: EditorSnapshot } | null {
	if (h.past.length === 0) return null;
	const target = h.past[h.past.length - 1];
	if (!target) return null;
	return {
		history: {
			past: h.past.slice(0, -1),
			future: [current, ...h.future].slice(0, MAX_HISTORY),
		},
		target,
	};
}

export function stepRedo(h: HistoryState, current: EditorSnapshot): { history: HistoryState; target: EditorSnapshot } | null {
	if (h.future.length === 0) return null;
	const target = h.future[0];
	if (!target) return null;
	return {
		history: {
			past: [...h.past, current].slice(-MAX_HISTORY),
			future: h.future.slice(1),
		},
		target,
	};
}
