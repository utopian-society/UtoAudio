<script lang="ts">
	// Vite default SVG import: returns a URL string (the SVG is served as a
	// static asset, NOT inlined into the JS bundle). This is the fallback
	// path when the `?raw` approach would push the bundle over budget.
	import logoSrc from '../assets/logo.svg';

	interface Props {
		/** Render size in CSS pixels (both width and height). */
		size?: number;
		/** Optional extra class names appended to the wrapping `<span>`. */
		class?: string;
	}

	let { size = 22, class: klass = '' }: Props = $props();
</script>

<span
	class="logo {klass}"
	style:width="{size}px"
	style:height="{size}px"
	aria-hidden="true"
>
	<img src={logoSrc} alt="" />
</span>

<style>
	.logo {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.logo img {
		width: 100%;
		height: 100%;
		display: block;
		object-fit: contain;
	}
</style>
