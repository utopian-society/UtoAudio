#!/usr/bin/env node
// Pre-builds the AMLL submodule's lyric + ttml parsers into self-contained
// ESM `.mjs` files with sibling `.d.mts` type declarations.
//
// Why: the upstream submodule is a pnpm workspace monorepo that needs its
// own toolchain (tsdown + Nx) to build, which can't run in this project's
// environment. The parser entry points used here (`lyric/src/formats/*` and
// `ttml/src/index`) have no external npm dependencies, so we bundle them
// directly with esbuild and consume the output via path imports.

import { build } from 'esbuild';
import { writeFileSync } from 'node:fs';

const SUB = 'src/lib/vendor/amll/packages';
const LRC = `${SUB}/lyric/src/formats/lrc.ts`;
const YRC = `${SUB}/lyric/src/formats/yrc.ts`;
const QRC = `${SUB}/lyric/src/formats/qrc.ts`;
const UTILS = `${SUB}/lyric/src/utils.ts`;
const TTML = `${SUB}/ttml/src/index.ts`;

const DIST_LYRIC = `${SUB}/lyric/dist`;
const DIST_TTML = `${SUB}/ttml/dist`;

const entries = [
	{ in: LRC, out: `${DIST_LYRIC}/formats-lrc.mjs` },
	{ in: YRC, out: `${DIST_LYRIC}/formats-yrc.mjs` },
	{ in: QRC, out: `${DIST_LYRIC}/formats-qrc.mjs` },
	{ in: UTILS, out: `${DIST_LYRIC}/utils.mjs` },
	{ in: TTML, out: `${DIST_TTML}/index.mjs` },
];

for (const { in: entry, out: outfile } of entries) {
	await build({
		entryPoints: [entry],
		bundle: true,
		format: 'esm',
		target: 'es2021',
		outfile,
		logLevel: 'info',
	});
}

// Type declarations (sibling .d.mts files — TypeScript picks these up
// automatically for the .mjs imports).
const wordShape = `export interface AmllLyricWord {
	startTime: number;
	endTime: number;
	word: string;
	romanWord?: string;
}`;
const lineShape = `export interface AmllLyricLine {
	words: AmllLyricWord[];
	translatedLyric: string;
	romanLyric: string;
	isBG: boolean;
	isDuet: boolean;
	startTime: number;
	endTime: number;
}`;
const lyricsDts = `${wordShape}
${lineShape}
export function parseLrc(content: string): AmllLyricLine[];
export function stringifyLrc(lines: AmllLyricLine[]): string;`;
writeFileSync(`${DIST_LYRIC}/formats-lrc.d.mts`, lyricsDts);

const yrcDts = `${wordShape}
${lineShape}
export function parseYrc(content: string): AmllLyricLine[];
export function stringifyYrc(lines: AmllLyricLine[]): string;`;
writeFileSync(`${DIST_LYRIC}/formats-yrc.d.mts`, yrcDts);

const qrcDts = `${wordShape}
${lineShape}
export function parseQrc(content: string): AmllLyricLine[];
export function stringifyQrc(lines: AmllLyricLine[]): string;`;
writeFileSync(`${DIST_LYRIC}/formats-qrc.d.mts`, qrcDts);

writeFileSync(
	`${DIST_LYRIC}/utils.d.mts`,
	`export const MAX_LRC_TIMESTAMP: number;\n`,
);

const ttmlDts = `export interface AmllLyricWordBase {
	startTime: number;
	endTime: number;
	word: string;
}
export interface AmllLyricWord extends AmllLyricWordBase {
	romanWord?: string;
	obscene?: boolean;
	emptyBeat?: number;
	ruby?: AmllLyricWordBase[];
}
export interface AmllLyricLine {
	words: AmllLyricWord[];
	translatedLyric: string;
	romanLyric: string;
	isBG: boolean;
	isDuet: boolean;
	startTime: number;
	endTime: number;
}
export type AmllMetadata = [string, string[]];
export interface AmllLyricResult {
	lines: AmllLyricLine[];
	metadata: AmllMetadata[];
}
export function parseTTML(content: string): AmllLyricResult;
export function exportTTML(lyric: AmllLyricResult): string;`;
writeFileSync(`${DIST_TTML}/index.d.mts`, ttmlDts);

console.log('AMLL submodule parsers built.');
