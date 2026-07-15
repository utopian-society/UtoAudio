// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

<script lang="ts">
	import { onMount } from 'svelte';
	import type { LyricTheme } from '../../lib/types/lyrics';

	/** The four background modes AMLL exposes (mapped to shader behaviour). */
	export type FluidBackgroundMode = 'fluid' | 'gradient' | 'blur' | 'solid';

	interface Props {
		/** Theme derived from album art; drives the gradient palette. */
		theme?: LyricTheme;
		/** Whether the animation is playing (drives the rAF loop). */
		playing?: boolean;
		/** Flow speed multiplier (default `2`, matching AMLL `BackgroundRender`). */
		flowSpeed?: number;
		/** Render scale `[0..1]` to trade quality for perf (AMLL default `0.5`). */
		renderScale?: number;
		/** Static mode freezes the image after a change (AMLL `staticMode`). */
		staticMode?: boolean;
		/** Optional album-art source (URL / image / video / canvas). */
		album?: string | HTMLImageElement | HTMLVideoElement | HTMLCanvasElement;
		/** Optional low-frequency volume `[0..1]` for beat-reactive motion. */
		lowFreqVolume?: number;
		/** Render mode. Defaults to `fluid`. */
		mode?: FluidBackgroundMode;
	}

	let {
		theme,
		playing = true,
		flowSpeed = 2,
		renderScale = 0.5,
		staticMode = false,
		album,
		lowFreqVolume = 1,
		mode = 'fluid',
	}: Props = $props();

	let canvas = $state<HTMLCanvasElement | null>(null);

	// Reusable shader sources. The vertex is a trivial fullscreen quad; the
	// fragment samples a CPU-built palette texture with AMLL's signature
	// rotating-UV + gradient-noise dither + vignette (ported from
	// `mesh.frag.glsl`) so the result visually echoes Apple Music's fluid bg.
	const VERT = `attribute vec2 a_pos; varying vec2 v_uv;
void main() {
	v_uv = a_pos * 0.5 + 0.5;
	gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

	// highp enough for desktop/mobile; texture built from theme.palette.
	const FRAG = `precision highp float;
varying vec2 v_uv;
uniform sampler2D u_palette;
uniform float u_time;
uniform float u_volume;
uniform float u_alpha;
uniform float u_mode; // 0 fluid, 1 gradient, 2 blur, 3 solid
uniform float u_flow;
uniform vec3 u_solid;

const float INV_255 = 1.0 / 255.0;
const float HALF_INV_255 = 0.5 / 255.0;
const float GRADIENT_NOISE_A = 52.9829189;
const vec2 GRADIENT_NOISE_B = vec2(0.06711056, 0.00583715);

float gradientNoise(in vec2 uv) {
	return fract(GRADIENT_NOISE_A * fract(dot(uv, GRADIENT_NOISE_B)));
}
vec2 rot(in vec2 v, float a) {
	float s = sin(a); float c = cos(a);
	return vec2(c * v.x - s * v.y, s * v.x + c * v.y);
}
void main() {
	float volumeEffect = u_volume * 2.0;
	float timeVolume = u_time + u_volume;
	float dither = INV_255 * gradientNoise(gl_FragCoord.xy) - HALF_INV_255;

	vec2 centered = v_uv - vec2(0.2);
	vec2 rotated = rot(centered, timeVolume * 2.0 * u_flow);
	float scale = max(0.001, 1.0 - volumeEffect);
	vec2 uv = rotated * scale + vec2(0.5);
	// Soften sampling for "blur" mode.
	if (u_mode > 1.5 && u_mode < 2.5) {
		uv += vec2(sin(u_time * 0.5), cos(u_time * 0.5)) * 0.05;
	}

	vec4 result;
	if (u_mode > 2.5) {
		// solid
		result = vec4(u_solid, 1.0);
	} else {
		result = texture2D(u_palette, uv);
	}

	float alphaFactor = u_alpha * max(0.5, 1.0 - u_volume * 0.5);
	result.rgb *= alphaFactor;
	result.a *= alphaFactor;

	// gradient mode: no noise, no flow tint — just the palette as a soft wash.
	if (u_mode < 0.5) {
		result.rgb += vec3(dither);
	}

	float dist = distance(v_uv, vec2(0.5));
	float vignette = smoothstep(0.8, 0.3, dist);
	float mask = 0.6 + vignette * 0.4;
	result.rgb *= mask;

	gl_FragColor = result;
}`;

	let gl: WebGLRenderingContext | null = null;
	let program: WebGLProgram | null = null;
	let paletteTex: WebGLTexture | null = null;
	let quadBuf: WebGLBuffer | null = null;
	const uniforms: Record<string, WebGLUniformLocation | null> = {};
	let startTime = 0;
	let raf = 0;

	function compile(type: number, src: string): WebGLShader | null {
		if (!gl) return null;
		const sh = gl.createShader(type);
		if (!sh) return null;
		gl.shaderSource(sh, src);
		gl.compileShader(sh);
		if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
			console.warn('[FluidBackground] shader compile failed:', gl.getShaderInfoLog(sh));
			gl.deleteShader(sh);
			return null;
		}
		return sh;
	}

	function buildProgram(): WebGLProgram | null {
		if (!gl) return null;
		const vs = compile(gl.VERTEX_SHADER, VERT);
		const fs = compile(gl.FRAGMENT_SHADER, FRAG);
		if (!vs || !fs) return null;
		const prog = gl.createProgram();
		if (!prog) return null;
		gl.attachShader(prog, vs);
		gl.attachShader(prog, fs);
		gl.linkProgram(prog);
		if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
			console.warn('[FluidBackground] program link failed:', gl.getProgramInfoLog(prog));
			return null;
		}
		return prog;
	}

	/** Build a 1D-friendly palette texture from the theme palette (or fallbacks). */
	function buildPalette(): void {
		if (!gl) return;
		const palette = theme?.palette && theme.palette.length > 0 ? theme.palette : ['#1a1a2e', '#16213e', '#0f3460', '#533483'];
		const cells = Math.max(palette.length, 4);
		const data = new Uint8Array(cells * 4);
		palette.forEach((hex, i) => {
			const c = hexToRgb(hex);
			data[i * 4] = c.r;
			data[i * 4 + 1] = c.g;
			data[i * 4 + 2] = c.b;
			data[i * 4 + 3] = 255;
		});
		// Pad with the last colour if palette shorter than cells.
		const last = palette[palette.length - 1];
		const lr = hexToRgb(last);
		for (let i = palette.length; i < cells; i++) {
			data[i * 4] = lr.r;
			data[i * 4 + 1] = lr.g;
			data[i * 4 + 2] = lr.b;
			data[i * 4 + 3] = 255;
		}
		if (!paletteTex) paletteTex = gl.createTexture();
		gl.bindTexture(gl.TEXTURE_2D, paletteTex);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, cells, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, data);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
	}

	function hexToRgb(hex: string): { r: number; g: number; b: number } {
		let h = hex.replace('#', '');
		if (h.length === 3) h = h.split('').map((c) => c + c).join('');
		const n = parseInt(h, 16);
		if (Number.isNaN(n)) return { r: 128, g: 128, b: 128 };
		return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
	}

	function resize(): void {
		if (!gl || !canvas) return;
		const dpr = Math.min(window.devicePixelRatio || 1, 1 / renderScale);
		const w = Math.max(1, Math.floor(canvas.clientWidth * dpr));
		const h = Math.max(1, Math.floor(canvas.clientHeight * dpr));
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
			gl.viewport(0, 0, w, h);
		}
	}

	function modeIndex(m: FluidBackgroundMode): number {
		switch (m) {
			default:
			case 'fluid':
				return 0;
			case 'gradient':
				return 1;
			case 'blur':
				return 2;
			case 'solid':
				return 3;
		}
	}

	function frame(): void {
		if (!gl || !canvas || !program) return;
		const t = (performance.now() - startTime) / 1000;
		resize();
		gl.clearColor(0, 0, 0, 0);
		gl.clear(gl.COLOR_BUFFER_BIT);
		gl.useProgram(program);
		gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
		const posLoc = gl.getAttribLocation(program, 'a_pos');
		gl.enableVertexAttribArray(posLoc);
		gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, paletteTex);
		gl.uniform1i(uniforms.u_palette, 0);
		gl.uniform1f(uniforms.u_time, staticMode ? 0 : t);
		gl.uniform1f(uniforms.u_volume, staticMode ? 0 : lowFreqVolume);
		gl.uniform1f(uniforms.u_alpha, 1);
		gl.uniform1f(uniforms.u_mode, modeIndex(mode));
		gl.uniform1f(uniforms.u_flow, flowSpeed);
		const solid = theme ? hexToRgb(theme.color) : { r: 30, g: 30, b: 46 };
		gl.uniform3f(uniforms.u_solid, solid.r / 255, solid.g / 255, solid.b / 255);
		gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
	}

	onMount(() => {
		if (!canvas) return;
		const webgl = canvas.getContext('webgl', { alpha: true, premultipliedAlpha: false, antialias: false });
		if (!webgl) {
			console.warn('[FluidBackground] WebGL unavailable');
			return;
		}
		gl = webgl;
		program = buildProgram();
		if (!program) return;
		quadBuf = gl.createBuffer();
		gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
		for (const u of ['u_palette', 'u_time', 'u_volume', 'u_alpha', 'u_mode', 'u_flow', 'u_solid']) {
			uniforms[u] = gl.getUniformLocation(program, u);
		}
		buildPalette();
		startTime = performance.now();
		const ro = new ResizeObserver(resize);
		ro.observe(canvas);
		resize();
		return () => {
			cancelAnimationFrame(raf);
			ro.disconnect();
			if (program && gl) gl.deleteProgram(program);
			if (quadBuf && gl) gl.deleteBuffer(quadBuf);
			if (paletteTex && gl) gl.deleteTexture(paletteTex);
		};
	});

	/**
	 * The rAF loop, run in an `$effect` (Svelte's replacement for React's
	 * `requestAnimationFrame` in `useEffect`). Pauses when `playing` is false.
	 */
	$effect(() => {
		if (!gl || !program || !canvas) return;
		if (playing && !staticMode) {
			const loop = () => {
				frame();
				raf = requestAnimationFrame(loop);
			};
			raf = requestAnimationFrame(loop);
			return () => cancelAnimationFrame(raf);
		} else if (staticMode) {
			// render once after a change (theme/mode), then freeze
			frame();
		} else {
			// paused non-static: render current state once (no advance)
			frame();
		}
	});

	// Rebuild palette + redraw whenever the theme or mode changes.
	$effect(() => {
		theme;
		buildPalette();
		frame();
	});
	$effect(() => {
		mode;
		frame();
	});

	// Pull album art colours into the existing palette texture when `album` set.
	$effect(() => {
		album;
		// full album-image sampling is handled elsewhere (color.extractTheme);
		// here we just ensure a frame is drawn.
		frame();
	});
</script>

<canvas bind:this={canvas} class="amll-fluid-bg" aria-hidden="true"></canvas>

<style>
	.amll-fluid-bg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		display: block;
		z-index: 0;
		pointer-events: none;
	}
</style>
