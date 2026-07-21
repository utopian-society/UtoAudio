// This file is part of utoaudio, licensed under AGPL-3.0.
// Consumes AMLL lyric parsers from the applemusic-like-lyrics submodule
// (https://github.com/utopian-society/applemusic-like-lyrics), which is
// also licensed under AGPL-3.0. See LICENSE for full license text.
//
// The submodule's source TypeScript uses .ts extension imports that are
// incompatible with this project's tsc settings (and its `core` package
// has external deps not in our package.json). To keep the submodule
// untouched we pre-bundle the four parser entry points with esbuild into
// the submodule's `dist/` directory (consumed here via .mjs + sibling
// `.d.mts` declarations).

import {
	parseLrc as parseLrcRaw,
	stringifyLrc,
} from '../vendor/amll/packages/lyric/dist/formats-lrc.mjs';
import {
	parseYrc as parseYrcRaw,
	stringifyYrc,
} from '../vendor/amll/packages/lyric/dist/formats-yrc.mjs';
import {
	parseQrc as parseQrcRaw,
	stringifyQrc,
} from '../vendor/amll/packages/lyric/dist/formats-qrc.mjs';
import { parseTTML as parseTTMLRaw } from '../vendor/amll/packages/ttml/dist/index.mjs';
import { MAX_LRC_TIMESTAMP } from '../vendor/amll/packages/lyric/dist/utils.mjs';
import type {
	AmllLyricLine,
	AmllLyricResult,
} from '../vendor/amll/packages/ttml/dist/index.d.mts';
import {
	type LyricLine,
	type LyricMetadata,
	type LyricSource,
	fromSimpleLyricLines,
} from '../types/lyrics';

export { MAX_LRC_TIMESTAMP };
export { fromSimpleLyricLines };

export { parseLrcRaw as parseLrc, stringifyLrc };
export { parseYrcRaw as parseYrc, stringifyYrc };
export { parseQrcRaw as parseQrc, stringifyQrc };

function adaptLine(line: AmllLyricLine): LyricLine {
	return {
		words: line.words.map((w) => ({
			startTime: w.startTime,
			endTime: w.endTime,
			word: w.word,
			romanWord: w.romanWord,
			obscene: w.obscene,
			ruby: w.ruby?.map((r) => ({
				startTime: r.startTime,
				endTime: r.endTime,
				text: r.word,
			})),
		})),
		translatedLyric: line.translatedLyric,
		romanLyric: line.romanLyric,
		startTime: line.startTime,
		endTime: line.endTime,
		isBG: line.isBG,
		isDuet: line.isDuet,
	};
}

export function parseTTML(ttmlText: string): LyricLine[] {
	const result: AmllLyricResult = parseTTMLRaw(ttmlText);
	return result.lines.map(adaptLine);
}

export function parseTTMLWithMetadata(ttmlText: string): {
	lines: LyricLine[];
	metadata?: LyricMetadata;
} {
	const result: AmllLyricResult = parseTTMLRaw(ttmlText);
	return {
		lines: result.lines.map(adaptLine),
		metadata: result.metadata.length
			? { entries: result.metadata }
			: undefined,
	};
}

export type LyricFormat = 'ttml' | 'lrc' | 'yrc' | 'qrc';

function finalizeEndTimes(lines: LyricLine[]): LyricLine[] {
	if (lines.length === 0) return lines;
	const last = lines[lines.length - 1];
	if (last.endTime <= last.startTime || last.endTime >= MAX_LRC_TIMESTAMP) {
		last.endTime = MAX_LRC_TIMESTAMP;
		if (last.words.length === 1) {
			last.words[0].endTime = Math.max(
				last.words[0].endTime,
				MAX_LRC_TIMESTAMP,
			);
		}
	}
	return lines;
}

/**
 * Parse lyric content using the AMLL parser appropriate for the format.
 * Format detection is now done in the Rust backend; this function just
 * dispatches to the right AMLL parser.
 */
export function parseLyrics(
	content: string,
	format: LyricFormat = 'ttml',
): Promise<LyricLine[]> {
	const run = (): LyricLine[] => {
		switch (format) {
			case 'ttml':
				return parseTTML(content);
			case 'yrc':
				return parseYrcRaw(content);
			case 'qrc':
				return parseQrcRaw(content);
			case 'lrc':
				// LRC should have been converted to TTML by the backend.
				// Fall back to AMLL LRC parser if we somehow receive raw LRC.
				return parseLrcRaw(content);
		}
	};
	return new Promise((resolve) => {
		if (typeof queueMicrotask === 'function') {
			queueMicrotask(() => resolve(finalizeEndTimes(run())));
		} else {
			resolve(finalizeEndTimes(run()));
		}
	});
}

export function parseLyricsFull(
	content: string,
	format: LyricFormat = 'ttml',
): Promise<LyricSource> {
	const run = (): LyricSource => {
		switch (format) {
			case 'ttml': {
				const { lines, metadata } = parseTTMLWithMetadata(content);
				return { lines: finalizeEndTimes(lines), metadata };
			}
			case 'yrc':
				return { lines: finalizeEndTimes(parseYrcRaw(content)) };
			case 'qrc':
				return { lines: finalizeEndTimes(parseQrcRaw(content)) };
			case 'lrc':
				return { lines: finalizeEndTimes(parseLrcRaw(content)) };
		}
	};
	return new Promise((resolve) => {
		if (typeof queueMicrotask === 'function') {
			queueMicrotask(() => resolve(run()));
		} else {
			resolve(run());
		}
	});
}