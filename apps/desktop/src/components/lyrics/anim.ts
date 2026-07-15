// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

/**
 * Shared rendering math utilities ported from AMLL `packages/core/src/utils/`.
 * Pure functions, no DOM / Svelte coupling.
 */

export function clamp(x: number, min: number, max: number): number {
	return Math.min(Math.max(x, min), max);
}

export function clamp01(x: number): number {
	return clamp(x, 0, 1);
}

export function clampPositive(x: number): number {
	return Math.max(0, x);
}

/** `easeOutExpo`: snappy ramp-in used by the interlude dots. */
export function easeOutExpo(x: number): number {
	return x >= 1 ? 1 : x <= 0 ? 0 : 1 - 2 ** (-10 * x);
}

/** `easeInOutBack`: the squeeze-out easing used by the interlude dots. */
export function easeInOutBack(x: number): number {
	const c1 = 1.70158;
	const c2 = c1 * 1.525;
	if (x < 0.5) {
		const t = 2 * x;
		return (t * t * ((c2 + 1) * t - c2)) / 2;
	}
	const t = 2 * x - 2;
	return (t * t * ((c2 + 1) * t + c2) + 2) / 2;
}

/**
 * Whether a single character is a CJK ideograph.
 * Ported from AMLL `packages/core/src/utils/is-cjk.ts`.
 */
const CJK_RE = /^[\p{Unified_Ideograph}\u0800-\u9FFC]+$/u;
export function isCJK(char: string): boolean {
	return CJK_RE.test(char);
}

/** Whether a single character is whitespace. */
const WHITESPACE_RE = /\s/;
export function isWhitespace(char: string): boolean {
	return WHITESPACE_RE.test(char);
}

/**
 * Lazily-initialized grapheme segmenter used to split words into graphemes
 * (handles emoji / ZWJ clusters). Falls back to per-code-point splitting if
 * `Intl.Segmenter` is unavailable.
 */
let _graphemes: Intl.Segmenter | null = null;
function getSegmenter(): Intl.Segmenter | null {
	if (_graphemes === null) {
		try {
			_graphemes =
				typeof Intl !== 'undefined' && 'Segmenter' in Intl
					? new Intl.Segmenter(undefined, { granularity: 'grapheme' })
					: null;
		} catch {
			_graphemes = null;
		}
	}
	return _graphemes;
}

/** Split a string into grapheme clusters. */
export function splitGraphemes(text: string): string[] {
	const seg = getSegmenter();
	if (seg) {
		return Array.from(seg.segment(text), (s) => s.segment);
	}
	return Array.from(text);
}

/**
 * Repeat a cubic-bezier easing centred on `mid` so the value rises to 1 at `mid`
 * and falls back to 0 by `1`. Used by the emphasize "bump" keyframes.
 */
export function makeEmpEasing(mid: number): (x: number) => number {
	const beginNum = (x: number) => x / mid;
	const endNum = (x: number) => (x - mid) / (1 - mid);
	const bezIn = (t: number) => bez(0.2, 0.4, 0.58, 1.0, t);
	const bezOut = (t: number) => bez(0.3, 0.0, 0.58, 1.0, t);
	return (x: number) =>
		x < mid ? bezIn(beginNum(x)) : 1 - bezOut(endNum(x));
}

/** Evaluate a cubic Bézier easing with `p1.y = 0, p4.y = 1` control points. */
export function bez(x1: number, y1: number, x2: number, y2: number, t: number): number {
	// Newton-Raphson root finding on the cubic's x-projection.
	const cx = 3 * x1;
	const bx = 3 * (x2 - x1) - cx;
	const ax = 1 - cx - bx;
	const cy = 3 * y1;
	const by = 3 * (y2 - y1) - cy;
	const ay = 1 - cy - by;
	let guess = t;
	for (let i = 0; i < 8; i++) {
		const x = ((ax * guess + bx) * guess + cx) * guess - t;
		if (Math.abs(x) < 1e-6) break;
		const d = (3 * ax * guess + 2 * bx) * guess + cx;
		if (Math.abs(d) < 1e-6) break;
		guess -= x / d;
	}
	return ((ay * guess + by) * guess + cy) * guess;
}
