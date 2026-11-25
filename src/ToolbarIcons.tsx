import type { ReactNode } from "react";

function Svg({ children }: { children: ReactNode }) {
	return (
		<svg
			width="16"
			height="16"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.75"
			strokeLinecap="round"
			strokeLinejoin="round"
			aria-hidden="true"
		>
			{children}
		</svg>
	);
}

/** Group 1: Tools */
export function IconHand() {
	return (
		<Svg>
			<path d="M18 11V6a2 2 0 0 0-4 0v4" />
			<path d="M14 10V4a2 2 0 0 0-4 0v6" />
			<path d="M10 10.5V6a2 2 0 0 0-4 0v8" />
			<path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.9-5.9-2.3L3.3 16.9a2 2 0 0 1 2.8-2.8L8 16" />
		</Svg>
	);
}

export function IconPointer() {
	return (
		<Svg>
			<path d="M4 3l7.2 17 2.5-6.3L20 11.2 4 3z" />
			<path d="M13.7 13.7L19 19" />
		</Svg>
	);
}

/** Node tool: classic Pen Tool nib. */
export function IconNode() {
	return (
		<Svg>
			<path d="M12 19l7-7 3 3-7 7-3-3z" />
			<path d="M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z" />
			<path d="M2 2l7.586 7.586" />
			<circle cx="11" cy="11" r="2" />
		</Svg>
	);
}

/** Detail brush: diagonal paintbrush with a '+' badge in the bottom-right corner. */
export function IconDetailBrush() {
	return (
		<Svg>
			<path d="M18.2 2.8a2.12 2.12 0 0 1 3 3L12.2 14.8l-3-3 9-9z" />
			<path d="M11.2 9.8l3 3" />
			<path d="M9.2 11.8c-2.2 0-4.2 1.6-4.2 4.2 0 1.4-.6 2.6-2 3.5 1.8.8 4.2.5 5.8-.7 1.7-1.2 2.9-2.4 3.4-4" />
			<path d="M18.5 15.5v6" />
			<path d="M15.5 18.5h6" />
		</Svg>
	);
}

/** Smooth brush: diagonal paintbrush with a smooth S-curve in the bottom-right corner. */
export function IconSmoothBrush() {
	return (
		<Svg>
			<path d="M18.2 2.8a2.12 2.12 0 0 1 3 3L12.2 14.8l-3-3 9-9z" />
			<path d="M11.2 9.8l3 3" />
			<path d="M9.2 11.8c-2.2 0-4.2 1.6-4.2 4.2 0 1.4-.6 2.6-2 3.5 1.8.8 4.2.5 5.8-.7 1.7-1.2 2.9-2.4 3.4-4" />
			<path d="M13.5 20c1.4-2.4 2.8-2.4 4.2 0s2.8 2.4 4.3 0" />
		</Svg>
	);
}

export function IconEyedropper() {
	return (
		<Svg>
			<path d="M2 22l1-1h3l9-9" />
			<path d="M3 21v-3l9-9" />
			<path d="M15 6l3.4-3.4a2.1 2.1 0 1 1 3 3L18 9l.4.4a2.1 2.1 0 1 1-3 3l-3.8-3.8a2.1 2.1 0 1 1 3-3L15 6z" />
		</Svg>
	);
}

/** Group 2: History */
export function IconUndo() {
	return (
		<Svg>
			<path d="M9 14L4 9l5-5" />
			<path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11" />
		</Svg>
	);
}

export function IconRedo() {
	return (
		<Svg>
			<path d="M15 14l5-5-5-5" />
			<path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13" />
		</Svg>
	);
}

/** Group 3: Preview & Pre-process */
export function IconOutline() {
	return (
		<Svg>
			<rect x="3" y="3" width="5" height="5" rx="1" />
			<rect x="16" y="16" width="5" height="5" rx="1" />
			<path d="M8 5.5C15 5.5 9 18.5 16 18.5" />
			<circle cx="17" cy="6" r="1.5" />
			<circle cx="7" cy="18" r="1.5" />
		</Svg>
	);
}

export function IconSoftProof() {
	return (
		<Svg>
			<path d="M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z" />
			<path d="M12 18a4 4 0 0 0 4-4" />
		</Svg>
	);
}

export function IconCrop() {
	return (
		<Svg>
			<path d="M6 2v14a2 2 0 0 0 2 2h14" />
			<path d="M18 22V8a2 2 0 0 0-2-2H2" />
		</Svg>
	);
}

export function IconHeatmap() {
	return (
		<Svg>
			<rect x="3" y="3" width="7" height="7" rx="1" />
			<rect x="14" y="3" width="7" height="7" rx="1" />
			<rect x="3" y="14" width="7" height="7" rx="1" />
			<rect x="14" y="14" width="7" height="7" rx="1" fill="currentColor" fillOpacity="0.35" />
		</Svg>
	);
}

export function IconChecker() {
	return (
		<Svg>
			<rect x="3" y="3" width="18" height="18" rx="2" />
			<rect x="3" y="3" width="6" height="6" fill="currentColor" stroke="none" />
			<rect x="15" y="3" width="6" height="6" fill="currentColor" stroke="none" />
			<rect x="9" y="9" width="6" height="6" fill="currentColor" stroke="none" />
			<rect x="3" y="15" width="6" height="6" fill="currentColor" stroke="none" />
			<rect x="15" y="15" width="6" height="6" fill="currentColor" stroke="none" />
		</Svg>
	);
}

export function IconGear() {
	return (
		<Svg>
			<circle cx="12" cy="12" r="3" />
			<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
		</Svg>
	);
}
