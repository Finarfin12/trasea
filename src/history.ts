import type { BrushStroke, ExportOptions, NodeEdit, Options, PathEdit } from "./api";

export type TargetChoice = "auto" | "500" | "1000" | "2000" | "custom";

export interface EditorSnapshot {
	opts: Options;
	palette: string[] | null;
	paletteMerges: Array<[string, string]>;
	strokes: BrushStroke[];
	pathEdits: PathEdit[];
	nodeEdits: NodeEdit[];
	targetChoice: TargetChoice;
	customTarget: string;
	exp: ExportOptions;
	label: string;
}

export interface HistoryState {
	past: EditorSnapshot[];
	future: EditorSnapshot[];
}

export const DEFAULT_MAX_HISTORY = 50;

export const emptyHistory = (): HistoryState => ({ past: [], future: [] });

export function pushSnapshot(h: HistoryState, prev: EditorSnapshot, maxSteps = DEFAULT_MAX_HISTORY): HistoryState {
	const cap = Math.max(1, maxSteps);
	const nextPast = [...h.past, prev];
	if (nextPast.length > cap) nextPast.splice(0, nextPast.length - cap);
	return { past: nextPast, future: [] };
}

export function stepUndo(
	h: HistoryState,
	current: EditorSnapshot,
	maxSteps = DEFAULT_MAX_HISTORY,
): { history: HistoryState; target: EditorSnapshot } | null {
	if (h.past.length === 0) return null;
	const target = h.past[h.past.length - 1];
	if (!target) return null;
	const cap = Math.max(1, maxSteps);
	return {
		history: {
			past: h.past.slice(0, -1),
			future: [current, ...h.future].slice(0, cap),
		},
		target,
	};
}

export function stepRedo(
	h: HistoryState,
	current: EditorSnapshot,
	maxSteps = DEFAULT_MAX_HISTORY,
): { history: HistoryState; target: EditorSnapshot } | null {
	if (h.future.length === 0) return null;
	const target = h.future[0];
	if (!target) return null;
	const cap = Math.max(1, maxSteps);
	return {
		history: {
			past: [...h.past, current].slice(-cap),
			future: h.future.slice(1),
		},
		target,
	};
}
