// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

/**
 * Album-art colour extraction.
 *
 * AMLL derives its fluid-background palette from album art inside the Pixi
 * `MeshGradientRenderer` (k-means on a downscaled image plus a CPalette preset).
 * To keep this native Svelte + raw-WebGL component dependency-free, this module
 * implements a lightweight palette extractor: downscale the image onto a small
 * canvas, bucket pixels by quantized RGB, keep the most-populated distinct
 * buckets as the palette, and pick the most-vivid of those as the accent.
 *
 * This closely approximates the visual intent (a handful of dominant,
 * well-saturated colours) without pulling in Pixi / a k-means dependency.
 */
import type { LyricTheme } from '../../lib/types/lyrics';

const SAMPLE_SIZE = 48;

function quantize(v: number, bits = 4): number {
	const step = 256 >> bits;
	return Math.min(255, Math.floor(v / step) * step + step / 2);
}

function rgbToHex(r: number, g: number, b: number): string {
	const h = (n: number) => n.toString(16).padStart(2, '0');
	return `#${h(r)}${h(g)}${h(b)}`;
}

function hsl(r: number, g: number, b: number): { h: number; s: number; l: number } {
	r /= 255;
	g /= 255;
	b /= 255;
	const max = Math.max(r, g, b);
	const min = Math.min(r, g, b);
	const l = (max + min) / 2;
	let h = 0;
	const s = max === min ? 0 : l > 0.5 ? (max - min) / (2 - max - min) : (max - min) / (max + min);
	if (max !== min) {
		const d = max - min;
		switch (max) {
			case r:
				h = (g - b) / d + (g < b ? 6 : 0);
				break;
			case g:
				h = (b - r) / d + 2;
				break;
			default:
				h = (r - g) / d + 4;
		}
		h /= 6;
	}
	return { h: h * 360, s, l };
}

/**
 * Extract a {@link LyricTheme} from an image source (URL / HTMLImageElement /
 * HTMLCanvasElement / HTMLVideoElement). Resolves `null` if the source cannot
 * be drawn (cross-origin, decode failure, …).
 */
export async function extractTheme(
	src: string | HTMLImageElement | HTMLCanvasElement | HTMLVideoElement,
): Promise<LyricTheme | null> {
	const canvas = document.createElement('canvas');
	canvas.width = SAMPLE_SIZE;
	canvas.height = SAMPLE_SIZE;
	const ctx = canvas.getContext('2d', { willReadFrequently: true });
	if (!ctx) return null;

	if (typeof src === 'string') {
		const img = new Image();
		// Blob URLs (from URL.createObjectURL) don't support CORS — setting
		// crossOrigin on them causes a decode failure. Only set it for
		// http(s) URLs where the canvas would otherwise be tainted.
		if (!src.startsWith('blob:')) img.crossOrigin = 'anonymous';
		img.src = src;
		try {
			await img.decode();
		} catch {
			return null;
		}
		try {
			ctx.drawImage(img, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
		} catch {
			return null;
		}
	} else if (src instanceof HTMLImageElement) {
		try {
			await src.decode();
			ctx.drawImage(src, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
		} catch {
			return null;
		}
	} else if (src instanceof HTMLVideoElement) {
		try {
			ctx.drawImage(src, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
		} catch {
			return null;
		}
	} else {
		ctx.drawImage(src, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
	}

	let data: Uint8ClampedArray;
	try {
		data = ctx.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data;
	} catch {
		// tainted canvas (cross-origin without CORS); bail.
		return null;
	}

	const buckets = new Map<string, { count: number; r: number; g: number; b: number }>();
	for (let i = 0; i < data.length; i += 4) {
		const a = data[i + 3];
		if (a < 128) continue; // transparent
		const r = quantize(data[i]);
		const g = quantize(data[i + 1]);
		const b = quantize(data[i + 2]);
		const key = `${r},${g},${b}`;
		const existing = buckets.get(key);
		if (existing) {
			existing.count++;
		} else {
			buckets.set(key, { count: 1, r, g, b });
		}
	}

	if (buckets.size === 0) return null;

	// Distinct buckets, sorted by population, kept until too similar to one kept.
	const sorted = Array.from(buckets.values()).sort((a, b) => b.count - a.count);
	const palette: { r: number; g: number; b: number }[] = [];
	const colorDist = (
		a: { r: number; g: number; b: number },
		b: { r: number; g: number; b: number },
	) => Math.abs(a.r - b.r) + Math.abs(a.g - b.g) + Math.abs(a.b - b.b);

	for (const cand of sorted) {
		if (palette.length >= 8) break;
		// Keep all colours — near-blacks and near-whites are brightened/
		// dimmed by the caller's `brightenColor` so they read as gradient
		// stops. Only skip exact duplicates (similarity threshold).
		if (palette.some((p) => colorDist(p, cand) < 40)) continue;
		palette.push(cand);
	}

	// Fallback: keep the top buckets regardless of similarity.
	if (palette.length < 3) {
		for (const cand of sorted) {
			if (palette.includes(cand)) continue;
			palette.push(cand);
			if (palette.length >= 3) break;
		}
	}

	// Accent: most-saturated palette colour, biased toward mid luminance.
	const accent = palette
		.map((c) => {
			const { s, l } = hsl(c.r, c.g, c.b);
			const score = s * 2 - Math.abs(0.55 - l);
			return { c, score };
		})
		.sort((a, b) => b.score - a.score)[0]?.c ?? sorted[0];

	const avgL = palette.reduce((acc, c) => acc + hsl(c.r, c.g, c.b).l, 0) / palette.length;

	return {
		color: rgbToHex(accent.r, accent.g, accent.b),
		palette: palette.map((c) => rgbToHex(c.r, c.g, c.b)),
		light: avgL > 0.6,
	};
}
