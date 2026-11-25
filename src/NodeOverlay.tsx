import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import type { NodePoint, PathNodeGeometry } from "./api";

/**
 * Node tool overlay (Trasea 1.3.0): anchors and Bezier handles of one path, drawn in source pixels on top of the
 * preview. Dragging runs at display rate (requestAnimationFrame) on a local copy of the geometry; exactly one edit is
 * committed on pointer-up, so each gesture is one undo step.
 *
 * Square anchor = Corner node, circle anchor = Smooth node. Alt+drag on a handle breaks the tangent (no mirroring).
 */

export interface NodeSelection {
	sub: number;
	idx: number;
}

interface Props {
	geometry: PathNodeGeometry | null;
	width: number;
	height: number;
	showAllHandles: boolean;
	selected: NodeSelection | null;
	onSelect: (sel: NodeSelection | null) => void;
	onLivePathD?: (targetId: string, d: string | null) => void;
	onMoveNode: (sub: number, idx: number, x: number, y: number) => void;
	onMoveHandle: (sub: number, idx: number, which: "in" | "out", x: number, y: number, mirrorSmooth: boolean) => void;
	onToggleSmooth: (sub: number, idx: number) => void;
}

type Pt = [number, number];

interface Drag {
	kind: "node" | "in" | "out";
	sub: number;
	idx: number;
	x: number;
	y: number;
	/** screen position where the gesture started, to tell a click from a drag */
	startClientX: number;
	startClientY: number;
	moved: boolean;
	alt: boolean;
}

/** Local copy of the geometry with the running drag applied (this is what is drawn while dragging). */
function applyDrag(geo: PathNodeGeometry, d: Drag | null): PathNodeGeometry {
	if (!d || !d.moved) return geo;
	return {
		...geo,
		subpaths: geo.subpaths.map(sp => {
			if (sp.subpathIndex !== d.sub) return sp;
			return {
				...sp,
				nodes: sp.nodes.map((n): NodePoint => {
					if (n.index !== d.idx) return n;
					if (d.kind === "node") {
						const dx = d.x - n.x;
						const dy = d.y - n.y;
						return {
							...n,
							x: d.x,
							y: d.y,
							handleIn: n.handleIn ? [n.handleIn[0] + dx, n.handleIn[1] + dy] : null,
							handleOut: n.handleOut ? [n.handleOut[0] + dx, n.handleOut[1] + dy] : null,
						};
					}
					const moved: Pt = [d.x, d.y];
					const mirror = n.isSmooth && !d.alt;
					const otherKey = d.kind === "in" ? "handleOut" : "handleIn";
					const other = n[otherKey];
					let nextOther = other;
					if (mirror && other) {
						// keep the length of the opposite handle, align it opposite to the dragged one
						const vx = n.x - moved[0];
						const vy = n.y - moved[1];
						const vl = Math.hypot(vx, vy) || 1;
						const ol = Math.hypot(other[0] - n.x, other[1] - n.y);
						nextOther = [n.x + (vx / vl) * ol, n.y + (vy / vl) * ol];
					}
					return d.kind === "in"
						? { ...n, handleIn: moved, handleOut: nextOther }
						: { ...n, handleOut: moved, handleIn: nextOther };
				}),
			};
		}),
	};
}

function subpathD(nodes: NodePoint[], closed: boolean): string {
	if (nodes.length === 0) return "";
	const f = (v: number) => v.toFixed(2);
	const first = nodes[0]!;
	let d = `M${f(first.x)} ${f(first.y)}`;
	const seg = (a: NodePoint, b: NodePoint) => {
		const o = a.handleOut ?? [a.x, a.y];
		const i = b.handleIn ?? [b.x, b.y];
		d += `C${f(o[0])} ${f(o[1])} ${f(i[0])} ${f(i[1])} ${f(b.x)} ${f(b.y)}`;
	};
	for (let k = 0; k + 1 < nodes.length; k++) seg(nodes[k]!, nodes[k + 1]!);
	if (closed && nodes.length > 1) {
		seg(nodes[nodes.length - 1]!, first);
		d += "Z";
	}
	return d;
}

export function NodeOverlay({ geometry, width, height, showAllHandles, selected, onSelect, onMoveNode, onMoveHandle, onToggleSmooth, onLivePathD }: Props) {
	const svgRef = useRef<SVGSVGElement | null>(null);
	const [pxScale, setPxScale] = useState(1); // screen px per source px
	const [drag, setDrag] = useState<Drag | null>(null);
	const dragRef = useRef<Drag | null>(null);
	const rafRef = useRef<number | null>(null);

	useEffect(() => {
		const el = svgRef.current;
		if (!el) return;
		const update = () => {
			const w = el.getBoundingClientRect().width;
			if (w > 0 && width > 0) setPxScale(w / width);
		};
		update();
		const ro = new ResizeObserver(update);
		ro.observe(el);
		return () => ro.disconnect();
	}, [width]);

	const targetId = geometry?.targetId;
	const cancelDrag = useCallback(() => {
		dragRef.current = null;
		if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
		rafRef.current = null;
		if (targetId) onLivePathD?.(targetId, null);
		setDrag(null);
	}, [targetId, onLivePathD]);

	useEffect(() => {
		const onEscape = (event: KeyboardEvent) => {
			if (event.key !== "Escape" || !dragRef.current) return;
			event.stopImmediatePropagation();
			cancelDrag();
		};
		window.addEventListener("keydown", onEscape, true);
		return () => {
			window.removeEventListener("keydown", onEscape, true);
			cancelDrag();
		};
	}, [cancelDrag]);

	const toImage = useCallback(
		(clientX: number, clientY: number): Pt => {
			const r = (svgRef.current as SVGSVGElement).getBoundingClientRect();
			return [((clientX - r.left) / r.width) * width, ((clientY - r.top) / r.height) * height];
		},
		[width, height],
	);

	const flush = useCallback(() => {
		rafRef.current = null;
		const active = dragRef.current;
		if (!active?.moved || !geometry) return;
		const live = applyDrag(geometry, active);
		onLivePathD?.(geometry.targetId, live.subpaths.map(sp => subpathD(sp.nodes, sp.closed)).join(" "));
		setDrag({ ...active });
	}, [geometry, onLivePathD]);

	const begin = (e: ReactPointerEvent<SVGElement>, kind: Drag["kind"], sub: number, idx: number, x: number, y: number) => {
		if (e.button !== 0) return;
		e.stopPropagation();
		(e.currentTarget as Element).setPointerCapture(e.pointerId);
		dragRef.current = { kind, sub, idx, x, y, startClientX: e.clientX, startClientY: e.clientY, moved: false, alt: e.altKey };
		onSelect({ sub, idx });
	};

	const move = (e: ReactPointerEvent<SVGElement>) => {
		const d = dragRef.current;
		if (!d) return;
		const [x, y] = toImage(e.clientX, e.clientY);
		const moved = d.moved || Math.hypot(e.clientX - d.startClientX, e.clientY - d.startClientY) > 3;
		dragRef.current = { ...d, x, y, moved, alt: e.altKey };
		if (rafRef.current === null) rafRef.current = requestAnimationFrame(flush);
	};

	const end = (e: ReactPointerEvent<SVGElement>) => {
		const d = dragRef.current;
		cancelDrag();
		if (!d || !d.moved) return;
		const [x, y] = toImage(e.clientX, e.clientY);
		if (d.kind === "node") onMoveNode(d.sub, d.idx, x, y);
		else {
			const node = geometry?.subpaths.find(s => s.subpathIndex === d.sub)?.nodes.find(n => n.index === d.idx);
			onMoveHandle(d.sub, d.idx, d.kind, x, y, !!node?.isSmooth && !e.altKey);
		}
	};

	const shown = useMemo(() => (geometry ? applyDrag(geometry, drag) : null), [geometry, drag]);
	if (!shown) return null;

	const r = 5 / pxScale; // anchor half-size / handle radius in source px
	const hr = 3.5 / pxScale;
	const sw = 1.25 / pxScale;

	return (
		<svg
			ref={svgRef}
			className="node-overlay"
			viewBox={`0 0 ${width} ${height}`}
			preserveAspectRatio="none"
			data-testid="node-overlay"
			aria-hidden="true"
		>
			{shown.subpaths.map(sp => (
				<g key={sp.subpathIndex}>
					<path d={subpathD(sp.nodes, sp.closed)} className="node-outline" strokeWidth={sw} fill="none" vectorEffect="non-scaling-stroke" />
					{sp.nodes.map(n => {
						const isSel = selected?.sub === sp.subpathIndex && selected.idx === n.index;
						const showHandles = showAllHandles || isSel;
						return (
							<g key={n.index}>
								{showHandles && n.handleIn && (
									<g>
										<line x1={n.x} y1={n.y} x2={n.handleIn[0]} y2={n.handleIn[1]} className="node-handle-line" strokeWidth={sw} />
										<circle
											cx={n.handleIn[0]}
											cy={n.handleIn[1]}
											r={hr}
											className="node-handle"
											data-handle="in"
											onPointerDown={e => begin(e, "in", sp.subpathIndex, n.index, n.handleIn![0], n.handleIn![1])}
											onPointerMove={move}
											onPointerUp={end}
											onPointerCancel={cancelDrag}
									onLostPointerCapture={cancelDrag}
										/>
									</g>
								)}
								{showHandles && n.handleOut && (
									<g>
										<line x1={n.x} y1={n.y} x2={n.handleOut[0]} y2={n.handleOut[1]} className="node-handle-line" strokeWidth={sw} />
										<circle
											cx={n.handleOut[0]}
											cy={n.handleOut[1]}
											r={hr}
											className="node-handle"
											data-handle="out"
											onPointerDown={e => begin(e, "out", sp.subpathIndex, n.index, n.handleOut![0], n.handleOut![1])}
											onPointerMove={move}
											onPointerUp={end}
											onPointerCancel={cancelDrag}
									onLostPointerCapture={cancelDrag}
										/>
									</g>
								)}
								{n.isSmooth ? (
									<circle
										cx={n.x}
										cy={n.y}
										r={r}
										className={`node-anchor smooth${isSel ? " selected" : ""}`}
										data-node={`${sp.subpathIndex}:${n.index}`}
										data-kind="smooth"
										onPointerDown={e => begin(e, "node", sp.subpathIndex, n.index, n.x, n.y)}
										onPointerMove={move}
										onPointerUp={end}
										onPointerCancel={cancelDrag}
									onLostPointerCapture={cancelDrag}
										onDoubleClick={() => onToggleSmooth(sp.subpathIndex, n.index)}
									/>
								) : (
									<rect
										x={n.x - r}
										y={n.y - r}
										width={2 * r}
										height={2 * r}
										className={`node-anchor corner${isSel ? " selected" : ""}`}
										data-node={`${sp.subpathIndex}:${n.index}`}
										data-kind="corner"
										onPointerDown={e => begin(e, "node", sp.subpathIndex, n.index, n.x, n.y)}
										onPointerMove={move}
										onPointerUp={end}
										onPointerCancel={cancelDrag}
									onLostPointerCapture={cancelDrag}
										onDoubleClick={() => onToggleSmooth(sp.subpathIndex, n.index)}
									/>
								)}
							</g>
						);
					})}
				</g>
			))}
		</svg>
	);
}
