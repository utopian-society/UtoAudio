<script lang="ts">
	// This file is part of utoaudio, licensed under AGPL-3.0.
	// Thin Svelte wrapper around the vendored AMLL `DomLyricPlayer` class.

	import { onMount } from 'svelte';
	import { DomLyricPlayer } from '../../lib/vendor/amll/packages/core/dist/lyric-player.mjs';
	import type { LyricLine as UtLyricLine } from '../../lib/types/lyrics';

	const ANCHOR_TOP = 'top';
	const ANCHOR_CENTER = 'center';
	const ANCHOR_BOTTOM = 'bottom';
	const MASK_DISABLED = '';

	interface AmllLyricWord {
		startTime: number;
		endTime: number;
		word: string;
		romanWord?: string;
		obscene?: boolean;
		emptyBeat?: number;
		ruby?: AmllLyricWord[];
	}
	interface AmllLyricLine {
		words: AmllLyricWord[];
		translatedLyric: string;
		romanLyric: string;
		isBG: boolean;
		isDuet: boolean;
		startTime: number;
		endTime: number;
	}
	interface PlayerInstance {
		readonly element: HTMLElement;
		setLyricLines(lines: AmllLyricLine[], initialTime?: number): void;
		setCurrentTime(time: number, isSeek?: boolean): void;
		setWordFadeWidth(value?: number): void;
		setEnableScale(enable?: boolean): void;
		setEnableBlur(enable?: boolean): void;
		setHidePassedLines(hide?: boolean): void;
		setEnableSpring(enable?: boolean): void;
		setAlignPosition(pos?: number): void;
		setAlignAnchor(anchor?: string): void;
		setIsSeeking(seeking?: boolean): void;
		setMaskObsceneWords(mode?: string): void;
		resume(): void;
		pause(): void;
		update(delta?: number): void;
		dispose(): void;
	}

	interface Props {
		lyrics: UtLyricLine[];
		currentTime: number;
		playing?: boolean;
		onLineChange?: (lineIndex: number) => void;
		alignPosition?: number;
		alignAnchor?: 'top' | 'bottom' | 'center';
		enableSpring?: boolean;
		enableBlur?: boolean;
		enableScale?: boolean;
		hidePassedLines?: boolean;
		wordFadeWidth?: number;
		isSeeking?: boolean;
		fontSize?: number;
		width?: number;
		height?: number;
		theme?: { color?: string; palette?: string[]; light?: boolean };
		enableFluidBackground?: boolean;
	}

	let {
		lyrics,
		currentTime = 0,
		playing = true,
		onLineChange,
		alignPosition = 0.35,
		alignAnchor = 'center',
		enableSpring = true,
		enableBlur = true,
		enableScale = true,
		hidePassedLines = false,
		wordFadeWidth = 0.5,
		isSeeking = false,
		fontSize: fontSizeProp,
		width,
		height,
		theme,
		enableFluidBackground = false,
	}: Props = $props();

	let containerEl = $state<HTMLDivElement | null>(null);
	let player: PlayerInstance | null = null;
	let rafId = 0;

	function toAmllLines(lines: UtLyricLine[]): AmllLyricLine[] {
		return lines.map((l) => ({
			words: l.words.map((w): AmllLyricWord => ({
				startTime: w.startTime,
				endTime: w.endTime,
				word: w.word,
				romanWord: w.romanWord,
				obscene: w.obscene,
				emptyBeat: undefined,
				ruby: w.ruby?.map((r) => ({
					startTime: r.startTime,
					endTime: r.endTime,
					word: r.text,
					romanWord: undefined,
					obscene: undefined,
					emptyBeat: undefined,
				})),
			})),
			translatedLyric: l.translatedLyric,
			romanLyric: l.romanLyric,
			isBG: l.isBG,
			isDuet: l.isDuet,
			startTime: l.startTime,
			endTime: l.endTime,
		}));
	}

	function anchorValue(a: string): string {
		return a === 'top' ? ANCHOR_TOP : a === 'bottom' ? ANCHOR_BOTTOM : ANCHOR_CENTER;
	}

	function tick(now: number, last: { current: number }): void {
		if (!player) return;
		const delta = now - last.current;
		last.current = now;
		player.update(delta);
		rafId = requestAnimationFrame((t) => tick(t, last));
	}

	onMount(() => {
		let disposed = false;

		function init() {
			if (!containerEl || disposed) return;
			const playerInstance = new DomLyricPlayer();
			player = playerInstance;
			containerEl.appendChild(player.element);
			const overrideStyle = document.createElement('style');
			const overrideCSS = [
				'.amll-lyric-player, .amll-lyric-player * {',
				'  mix-blend-mode: normal !important;',
				'  background: transparent !important;',
				'  color: #ffffff !important;',
				'}',
			].join('\n');
			overrideStyle.textContent = overrideCSS;
			player.element.insertBefore(overrideStyle, player.element.firstChild);
			player.setEnableSpring(enableSpring);
			player.setEnableBlur(enableBlur);
			player.setEnableScale(enableScale);
			player.setHidePassedLines(hidePassedLines);
			player.setWordFadeWidth(wordFadeWidth);
			player.setAlignPosition(alignPosition);
			player.setAlignAnchor(anchorValue(alignAnchor));
			player.setMaskObsceneWords(MASK_DISABLED);
			player.setLyricLines(toAmllLines(lyrics), currentTime);
			player.resume();
			const last = { current: performance.now() };
			rafId = requestAnimationFrame((t) => tick(t, last));
		}

		init();
		return () => {
			disposed = true;
			cancelAnimationFrame(rafId);
			player?.dispose();
			player = null;
		};
	});

	let prevLyricLines: UtLyricLine[] | null = null;

	$effect(() => {
		if (!player || !lyrics) return;
		const linesChanged = !prevLyricLines || lyrics.length !== prevLyricLines.length || lyrics.some((l, i) => l !== prevLyricLines[i]);
		if (!linesChanged) return;
		prevLyricLines = [...lyrics];
		player.setLyricLines(toAmllLines(lyrics), currentTime);
	});

	$effect(() => {
		player?.setCurrentTime(currentTime, isSeeking);
	});

	$effect(() => {
		player?.setEnableScale(enableScale);
		player?.setEnableBlur(enableBlur);
		player?.setEnableSpring(enableSpring);
		player?.setHidePassedLines(hidePassedLines);
		player?.setWordFadeWidth(wordFadeWidth);
	});

	$effect(() => {
		player?.setAlignPosition(alignPosition);
		player?.setAlignAnchor(anchorValue(alignAnchor));
	});

	$effect(() => {
		if (!containerEl) return;
		// Keep lyric text neutral white; the album-art theme drives only the
		// fluid background, not the lyric foreground.
		containerEl.style.setProperty('--amll-lp-color', '#ffffff');
		containerEl.style.setProperty('color', '#ffffff');
		containerEl.style.setProperty('mix-blend-mode', 'normal');
		containerEl.style.setProperty('isolation', 'isolate');
	});

	const fontSize = $derived(
		fontSizeProp
			? `${fontSizeProp}px`
			: (typeof window !== 'undefined' && window.matchMedia('(max-width: 768px)').matches
				? 'max(8vw, 12px)'
				: 'max(max(5vh, 2.5vw), 12px)'),
	);
</script>

<div bind:this={containerEl} class="amll-lyric-player-wrapper" style:--amll-lp-font-size={fontSize} style:width={width != null ? `${width}px` : null} style:height={height != null ? `${height}px` : null}></div>

<style>
	.amll-lyric-player-wrapper {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
		font-size: var(--amll-lp-font-size, max(max(5vh, 2.5vw), 12px));
	}

	:global(.amll-lyric-player-wrapper .amll-lyric-player),
	:global(.amll-lyric-player-wrapper .amll-lyric-player *) {
		color: #ffffff !important;
		mix-blend-mode: normal !important;
	}
</style>
