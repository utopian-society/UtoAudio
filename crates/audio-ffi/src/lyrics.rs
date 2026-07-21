// This file is part of utoaudio, licensed under AGPL-3.0.
//
// Lyric format detection, LRC→TTML conversion, and loading (embedded + sidecar).
// All LRC variants (standard, old-style inline, enhanced word-synced, bilingual)
// are converted to TTML so the frontend AMLL TTML parser handles them natively.
// YRC / QRC / TTML pass through unchanged — AMLL parses those natively.

use std::path::PathBuf;

use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use serde::{Deserialize, Serialize};

/// Sentinel end time for the last lyric line (matches frontend MAX_LRC_TIMESTAMP:
/// 999:59.999 in milliseconds).
const SENTINEL_END_MS: u64 = 60_039_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricFormat {
	Ttml,
	Yrc,
	Qrc,
	Lrc,
}

impl LyricFormat {
	pub fn as_str(self) -> &'static str {
		match self {
			LyricFormat::Ttml => "ttml",
			LyricFormat::Yrc => "yrc",
			LyricFormat::Qrc => "qrc",
			LyricFormat::Lrc => "lrc",
		}
	}
}

/// Payload returned to the frontend: a format tag + content string.
/// - `format = "ttml"`: content is TTML XML (converted from LRC or passed through)
/// - `format = "yrc"`:  content is raw YRC
/// - `format = "qrc"`:  content is raw QRC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricPayload {
	pub format: String,
	pub content: String,
}

// ───────────────────────── Parsed LRC structures ─────────────────────────

struct LrcLine {
	start_ms: u64,
	end_ms: u64,
	words: Vec<LrcWord>,
	text: String,
	translation: Option<String>,
}

struct LrcWord {
	start_ms: u64,
	end_ms: u64,
	text: String,
	/// Whether the original LRC source had trailing whitespace after this word.
	/// When `true`, the TTML emitter inserts a ` ` text-node between spans
	/// (mirroring AMLL's `Syllable.endsWithSpace` → `createTextNode(" ")`).
	/// For CJK word-synced LRC where words are packed with no space (e.g.
	/// `[00:12.34]這是[00:12.80]逐[00:13.10]字歌`), this stays `false` and
	/// no separator is emitted — words render packed as the source dictates.
	ends_with_space: bool,
}

struct LrcMetadata {
	title: Option<String>,
	artist: Option<String>,
	album: Option<String>,
}

// ───────────────────────────── Public API ────────────────────────────────

/// Load lyrics for an audio file: tries embedded tags first, then a sidecar
/// `.lrc` file. Converts LRC variants → TTML; passes YRC/QRC/TTML through.
/// Returns `Ok(None)` when no lyrics are found.
pub fn load_lyrics(path: &str) -> Result<Option<LyricPayload>, String> {
	if let Some(content) = read_embedded_lyrics(path)? {
		return Ok(Some(convert_to_payload(&content)));
	}
	let lrc_path = derive_lrc_path(path);
	if lrc_path.is_file() {
		let content = std::fs::read_to_string(&lrc_path).map_err(|e| e.to_string())?;
		return Ok(Some(convert_to_payload(&content)));
	}
	Ok(None)
}

fn convert_to_payload(content: &str) -> LyricPayload {
	match detect_format(content) {
		LyricFormat::Lrc => LyricPayload {
			format: "ttml".to_string(),
			content: lrc_to_ttml(content),
		},
		fmt => LyricPayload {
			format: fmt.as_str().to_string(),
			content: content.to_string(),
		},
	}
}

// ─────────────────────────── Format detection ────────────────────────────

pub fn detect_format(content: &str) -> LyricFormat {
	let head: &str = content
		.char_indices()
		.nth(512)
		.map(|(i, _)| &content[..i])
		.unwrap_or(content);
	let trimmed = head.trim_start();
	if trimmed.starts_with("<?xml") || trimmed.starts_with("<tt") {
		return LyricFormat::Ttml;
	}
	if content.contains("<tt") {
		return LyricFormat::Ttml;
	}
	// YRC / QRC header: [digits,digits] at line start
	let has_numeric_header = content.lines().take(10).any(|l| {
		let t = l.trim_start();
		if !t.starts_with('[') {
			return false;
		}
		let close = match t.find(']') {
			Some(i) => i,
			None => return false,
		};
		let inside = &t[1..close];
		inside.contains(',') && inside.chars().all(|c| c.is_ascii_digit() || c == ',')
	});
	if has_numeric_header && content.contains(",0)") {
		return LyricFormat::Yrc;
	}
	if has_numeric_header && content.contains("](") {
		return LyricFormat::Qrc;
	}
	LyricFormat::Lrc
}

// ─────────────────────────── LRC → TTML conversion ───────────────────────

fn lrc_to_ttml(content: &str) -> String {
	let offset = parse_offset(content);
	let metadata = parse_lrc_metadata(content);

	let mut lines: Vec<LrcLine> = Vec::new();
	for line in content.lines() {
		lines.extend(parse_lrc_line(line));
	}

	if offset != 0 {
		apply_offset(&mut lines, offset);
	}

	lines.sort_by_key(|l| l.start_ms);
	compute_end_times(&mut lines);
	let lines = merge_translations(lines);

	generate_ttml(&lines, &metadata)
}

fn parse_offset(content: &str) -> i64 {
	for line in content.lines() {
		let line = line.trim();
		if !line.starts_with('[') {
			continue;
		}
		let close = match line.find(']') {
			Some(i) => i,
			None => continue,
		};
		let inside = &line[1..close];
		if let Some(value) = inside.strip_prefix("offset:") {
			return value.trim().parse::<i64>().unwrap_or(0);
		}
	}
	0
}

fn parse_lrc_metadata(content: &str) -> LrcMetadata {
	let mut meta = LrcMetadata {
		title: None,
		artist: None,
		album: None,
	};
	for line in content.lines() {
		let line = line.trim();
		if !line.starts_with('[') {
			continue;
		}
		let close = match line.find(']') {
			Some(i) => i,
			None => continue,
		};
		let inside = &line[1..close];
		if let Some(v) = inside.strip_prefix("ti:") {
			meta.title = Some(v.trim().to_string());
		} else if let Some(v) = inside.strip_prefix("ar:") {
			meta.artist = Some(v.trim().to_string());
		} else if let Some(v) = inside.strip_prefix("al:") {
			meta.album = Some(v.trim().to_string());
		}
	}
	meta
}

/// Parse a single physical LRC line into one or more `LrcLine`s.
/// Handles: metadata tags (skipped), multi-timestamp lines, enhanced LRC
/// (`<ts>word`), old-style inline LRC (`[ts]word[ts]word`), standard LRC.
fn parse_lrc_line(line: &str) -> Vec<LrcLine> {
	let line = line.trim();
	if line.is_empty() || !line.starts_with('[') {
		return vec![];
	}

	let close = match line.find(']') {
		Some(i) => i,
		None => return vec![],
	};
	let first_inside = &line[1..close];
	if !is_timestamp(first_inside) {
		return vec![];
	}
	let first_ms = match parse_lrc_timestamp(first_inside) {
		Some(ms) => ms,
		None => return vec![],
	};

	let after_first = &line[close + 1..];

	// Multi-timestamp standard LRC: [ts][ts]text (no text between timestamps)
	if after_first.starts_with('[') {
		if let Some(second_close) = after_first.find(']') {
			let second_inside = &after_first[1..second_close];
			if is_timestamp(second_inside) {
				let mut timestamps = vec![first_ms];
				let mut rest = after_first;
				while rest.starts_with('[') {
					let c = match rest.find(']') {
						Some(i) => i,
						None => break,
					};
					let inside = &rest[1..c];
					if !is_timestamp(inside) {
						break;
					}
					match parse_lrc_timestamp(inside) {
						Some(ms) => {
							timestamps.push(ms);
							rest = &rest[c + 1..];
						}
						None => break,
					}
				}
				let text = rest.to_string();
				return timestamps
					.into_iter()
					.map(|ts| LrcLine {
						start_ms: ts,
						end_ms: 0,
						words: vec![],
						text: text.clone(),
						translation: None,
					})
					.collect();
			}
		}
	}

	// Enhanced LRC: <ts>word <ts>word ...
	if after_first.contains('<') && after_first.contains('>') {
		let words = parse_enhanced_words(after_first);
		if !words.is_empty() {
			return vec![LrcLine {
				start_ms: first_ms,
				end_ms: 0,
				words,
				text: String::new(),
				translation: None,
			}];
		}
	}

	// Old-style inline LRC: [ts]word[ts]word ...
	if after_first.contains('[') {
		let words = parse_inline_words(after_first, first_ms);
		if !words.is_empty() {
			return vec![LrcLine {
				start_ms: first_ms,
				end_ms: 0,
				words,
				text: String::new(),
				translation: None,
			}];
		}
	}

	// Standard LRC: [ts]text
	vec![LrcLine {
		start_ms: first_ms,
		end_ms: 0,
		words: vec![],
		text: after_first.to_string(),
		translation: None,
	}]
}

/// Parse enhanced (word-synced) LRC: `<mm:ss.xx>word <mm:ss.xx>word ...`
/// Each word's `ends_with_space` flag is set when the source text following
/// the word had trailing whitespace before the next `<timestamp>` (mirroring
/// AMLL's `Syllable.endsWithSpace` → TTML ` ` text-node emission).
fn parse_enhanced_words(content: &str) -> Vec<LrcWord> {
	let mut words = Vec::new();
	let mut rest = content;
	while let Some(open) = rest.find('<') {
		let close = match rest[open..].find('>') {
			Some(i) => open + i,
			None => break,
		};
		let ts_str = &rest[open + 1..close];
		let start_ms = match parse_lrc_timestamp(ts_str) {
			Some(ms) => ms,
			None => {
				rest = &rest[close + 1..];
				continue;
			}
		};
		rest = &rest[close + 1..];
		let word_end = rest.find('<').unwrap_or(rest.len());
		let raw_word = &rest[..word_end];
		let ends_with_space = raw_word.ends_with(char::is_whitespace);
		let word_text = raw_word.trim().to_string();
		if !word_text.is_empty() {
			words.push(LrcWord {
				start_ms,
				end_ms: 0,
				text: word_text,
				ends_with_space,
			});
		}
		rest = &rest[word_end..];
	}

	// AMLL `finalizeWords` resets the last word's `endsWithSpace=false`.
	if let Some(last) = words.last_mut() {
		last.ends_with_space = false;
	}
	words
}

/// Parse old-style inline LRC: `[mm:ss.xx]word[mm:ss.xx]word ...`
/// After extracting the line timestamp, the remainder looks like:
/// `word1[mm:ss.xx]word2[mm:ss.xx]word3...`
/// First word has no preceding timestamp (uses line start time).
///
/// `ends_with_space` on each word is set from the original source's trailing
/// whitespace — `trim_start()` only, so trailing space is preserved as a flag
/// (the stored `text` itself is fully trimmed; the flag is later used to emit
/// a ` ` text-node between TTML spans, mirroring AMLL's `Syllable.endsWithSpace`).
fn parse_inline_words(content: &str, line_start_ms: u64) -> Vec<LrcWord> {
	let mut words = Vec::new();
	let mut rest = content;

	// First word: text before the first '[' (uses line_start_ms)
	if let Some(first_bracket) = rest.find('[') {
		let raw = &rest[..first_bracket];
		let ends_with_space = raw.ends_with(char::is_whitespace);
		let trimmed = raw.trim();
		if !trimmed.is_empty() {
			words.push(LrcWord {
				start_ms: line_start_ms,
				end_ms: 0,
				text: trimmed.to_string(),
				ends_with_space,
			});
		}
		rest = &rest[first_bracket..];
	} else {
		// No more timestamps - entire rest is the first (and only) word.
		// The last word of a line never carries a trailing-space flag (matches
		// AMLL's `finalizeWords` which resets `endsWithSpace=false` on the last
		// word of every line).
		let word_text = rest.trim().to_string();
		if !word_text.is_empty() {
			words.push(LrcWord {
				start_ms: line_start_ms,
				end_ms: 0,
				text: word_text,
				ends_with_space: false,
			});
		}
		return words;
	}

	// Subsequent words: each preceded by [timestamp]
	while rest.starts_with('[') {
		let close = match rest.find(']') {
			Some(i) => i,
			None => break,
		};
		let ts_str = &rest[1..close];
		let start_ms = match parse_lrc_timestamp(ts_str) {
			Some(ms) => ms,
			None => {
				rest = &rest[close + 1..];
				continue;
			}
		};
		rest = &rest[close + 1..];
		let word_end = rest.find('[').unwrap_or(rest.len());
		let raw_word = &rest[..word_end];
		let ends_with_space = raw_word.ends_with(char::is_whitespace);
		let word_text = raw_word.trim().to_string();
		if !word_text.is_empty() {
			words.push(LrcWord {
				start_ms,
				end_ms: 0,
				text: word_text,
				ends_with_space,
			});
		}
		rest = &rest[word_end..];
	}

	// AMLL `finalizeWords` resets the last word's `endsWithSpace=false`.
	if let Some(last) = words.last_mut() {
		last.ends_with_space = false;
	}
	words
}

/// Returns `true` if `s` looks like an LRC timestamp (`mm:ss.xx`) rather than
/// a metadata tag (`ar:Artist`). The part before the first `:` must be all
/// digits.
fn is_timestamp(s: &str) -> bool {
	let parts: Vec<&str> = s.split(':').collect();
	if parts.len() < 2 {
		return false;
	}
	!parts[0].is_empty() && parts[0].chars().all(|c| c.is_ascii_digit())
}

/// Parse `mm:ss.xx` / `mm:ss.xxx` / `mm:ss` → milliseconds.
fn parse_lrc_timestamp(s: &str) -> Option<u64> {
	let parts: Vec<&str> = s.split(':').collect();
	if parts.len() != 2 {
		return None;
	}
	let minutes: u64 = parts[0].parse().ok()?;
	let sec_parts: Vec<&str> = parts[1].split('.').collect();
	let seconds: u64 = sec_parts[0].parse().ok()?;
	let millis: u64 = if sec_parts.len() > 1 {
		let frac = sec_parts[1];
		match frac.len() {
			0 => 0,
			1 => frac.parse::<u64>().ok()? * 100,
			2 => frac.parse::<u64>().ok()? * 10,
			3 => frac.parse::<u64>().ok()?,
			_ => frac[..3].parse::<u64>().ok()?,
		}
	} else {
		0
	};
	Some((minutes * 60 + seconds) * 1000 + millis)
}

fn apply_offset(lines: &mut Vec<LrcLine>, offset: i64) {
	for line in lines.iter_mut() {
		line.start_ms = (line.start_ms as i64 + offset).max(0) as u64;
		for word in line.words.iter_mut() {
			word.start_ms = (word.start_ms as i64 + offset).max(0) as u64;
		}
	}
}

/// Compute end times: each line's end = next line's start; each word's end =
/// next word's start. The last line gets the sentinel end time.
fn compute_end_times(lines: &mut Vec<LrcLine>) {
	let n = lines.len();
	for i in 0..n {
		let line_end = if i + 1 < n {
			lines[i + 1].start_ms
		} else {
			SENTINEL_END_MS
		};
		lines[i].end_ms = line_end;
		let wn = lines[i].words.len();
		for j in 0..wn {
			lines[i].words[j].end_ms = if j + 1 < wn {
				lines[i].words[j + 1].start_ms
			} else {
				line_end
			};
		}
	}
}

/// Merge bilingual LRC: consecutive lines with the same `start_ms`.
/// - Both line-level: second becomes translation of first.
/// - First word-level, second line-level: second becomes translation of first.
fn merge_translations(lines: Vec<LrcLine>) -> Vec<LrcLine> {
	let mut out: Vec<LrcLine> = Vec::new();
	for line in lines {
		if let Some(prev) = out.last_mut() {
			if prev.start_ms == line.start_ms && prev.translation.is_none() && !line.text.is_empty() {
				// Case 1: both line-level (standard bilingual LRC)
				// Case 2: first word-level, second line-level
				if (prev.words.is_empty() && line.words.is_empty())
					|| (!prev.words.is_empty() && line.words.is_empty())
				{
					prev.translation = Some(line.text.clone());
					prev.end_ms = prev.end_ms.max(line.end_ms);
					continue;
				}
			}
		}
		out.push(line);
	}
	out
}

/// Generate the TTML XML document from parsed LRC lines + metadata.
fn generate_ttml(lines: &[LrcLine], metadata: &LrcMetadata) -> String {
	let mut xml = String::new();
	xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
	// Check if any line has word-level timing
	let has_word_timing = lines.iter().any(|l| !l.words.is_empty());
	let timing_attr = if has_word_timing { " itunes:timing=\"Word\"" } else { "" };
	xml.push_str(
		&format!(
			"<tt xmlns=\"http://www.w3.org/ns/ttml\" \
			 xmlns:ttm=\"http://www.w3.org/ns/ttml#metadata\" \
			 xmlns:itunes=\"http://music.apple.com/lyric-ttml-internal\" \
			 xmlns:amll=\"http://www.example.com/ns/amll\" \
			 xml:lang=\"und\"{timing_attr}>\n",
		),
	);
	xml.push_str("<head><metadata>");
	xml.push_str("<ttm:agent type=\"person\" xml:id=\"v1\"/>");
	if let Some(ref title) = metadata.title {
		xml.push_str(&format!(
			"<amll:meta key=\"musicName\" value=\"{}\"/>",
			escape_xml(title)
		));
	}
	if let Some(ref artist) = metadata.artist {
		xml.push_str(&format!(
			"<amll:meta key=\"artists\" value=\"{}\"/>",
			escape_xml(artist)
		));
	}
	if let Some(ref album) = metadata.album {
		xml.push_str(&format!(
			"<amll:meta key=\"album\" value=\"{}\"/>",
			escape_xml(album)
		));
	}
	xml.push_str("</metadata></head>\n");
	xml.push_str("<body><div>\n");

	for (i, line) in lines.iter().enumerate() {
		let begin = format_time(line.start_ms);
		let end = format_time(line.end_ms);
		let key = format!("L{}", i + 1);
		xml.push_str(&format!(
			"<p begin=\"{}\" end=\"{}\" itunes:key=\"{}\" ttm:agent=\"v1\">",
			begin,
			end,
			key
		));

		if line.words.is_empty() {
			// Line-level lyric — text directly in <p>
			xml.push_str(&escape_xml(&line.text));
		} else {
			// Word-level lyric — `<span>` per word. A ` ` text-node is inserted
			// between spans when (and only when) the source LRC word carried
			// trailing whitespace (mirroring AMLL's `Syllable.endsWithSpace` →
			// `createTextNode(" ")` in the upstream generator). CJK word-synced
			// LRC (`[ts]這是[ts]逐[ts]字歌`) has no whitespace between words,
			// so no separator is emitted — words render packed, as the source dictates.
			for (j, word) in line.words.iter().enumerate() {
				xml.push_str(&format!(
					"<span begin=\"{}\" end=\"{}\">{}</span>",
					format_time(word.start_ms),
					format_time(word.end_ms),
					escape_xml(&word.text)
				));
				if j + 1 < line.words.len() && word.ends_with_space {
					xml.push(' ');
				}
			}
		}

		if let Some(ref trans) = line.translation {
			xml.push_str(&format!(
				"<span ttm:role=\"x-translation\" xml:lang=\"und\">{}</span>",
				escape_xml(trans)
			));
		}

		xml.push_str("</p>\n");
	}

	xml.push_str("</div></body>\n</tt>");
	xml
}

/// Format milliseconds as a TTML timecode.
/// - `< 60 min` → `MM:SS.fff`
/// - `>= 60 min` → `HH:MM:SS.fff`
/// - `>= SENTINEL` → `SS.fff` (seconds-only, avoids invalid MM >= 60)
fn format_time(ms: u64) -> String {
	if ms >= SENTINEL_END_MS {
		let secs = ms / 1000;
		let millis = ms % 1000;
		return format!("{}.{:03}", secs, millis);
	}
	let total_seconds = ms / 1000;
	let millis = ms % 1000;
	let seconds = total_seconds % 60;
	let total_minutes = total_seconds / 60;
	let minutes = total_minutes % 60;
	let hours = total_minutes / 60;
	if hours > 0 {
		format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
	} else {
		format!("{:02}:{:02}.{:03}", minutes, seconds, millis)
	}
}

fn escape_xml(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for c in s.chars() {
		match c {
			'&' => out.push_str("\x26amp;"),
			'<' => out.push_str("\x26lt;"),
			'>' => out.push_str("\x26gt;"),
			'"' => out.push_str("\x26quot;"),
			'\'' => out.push_str("\x26apos;"),
			_ => out.push(c),
		}
	}
	out
}

/// Derive the sidecar `.lrc` path from an audio file path.
fn derive_lrc_path(audio_path: &str) -> PathBuf {
	let path = PathBuf::from(audio_path);
	let stem = path.file_stem().unwrap_or_default();
	let mut lrc = path.with_file_name(stem);
	lrc.set_extension("lrc");
	lrc
}

/// Read embedded lyrics from audio file tags via `lofty`.
fn read_embedded_lyrics(path: &str) -> Result<Option<String>, String> {
	let parse_options = ParseOptions::new().read_properties(false);
	let tagged_file = Probe::open(path)
		.map_err(|e| e.to_string())?
		.options(parse_options)
		.guess_file_type()
		.map_err(|e| e.to_string())?
		.read()
		.map_err(|e| e.to_string())?;
	let tag = match tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
		Some(t) => t,
		None => return Ok(None),
	};
	Ok(tag.get_string(&ItemKey::Lyrics).map(|s| s.to_string()))
}

// ─────────────────────────────── Tests ───────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_standard_lrc() {
		let content =
			"[00:12.34]Never gonna give you up\n[00:15.67]Never gonna let you down";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("<tt"));
		assert!(ttml.contains("begin=\"00:12.340\""));
		assert!(ttml.contains(">Never gonna give you up</p>"));
		assert!(ttml.contains("itunes:key=\"L1\""));
		assert!(ttml.contains("ttm:agent=\"v1\""));
	}

	#[test]
	fn test_inline_lrc() {
		let content =
			"[00:12.34]Never[00:12.80]gonna[00:13.15]give[00:13.50]you[00:13.80]up";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains(
			"<span begin=\"00:12.340\" end=\"00:12.800\">Never</span>"
		));
		assert!(ttml.contains(
			"<span begin=\"00:12.800\" end=\"00:13.150\">gonna</span>"
		));
		assert!(ttml.contains(
			"<span begin=\"00:13.150\" end=\"00:13.500\">give</span>"
		));
		// No whitespace in the source → no ` ` separator between spans.
		assert!(!ttml.contains("</span> <span"));
	}

	#[test]
	fn test_inline_lrc_cjk() {
		// CJK word-synced LRC: no whitespace anywhere in the source — the
		// TTML emitter must NOT emit a ` ` text-node between spans.
		let content = "[00:12.34]這是[00:12.80]逐[00:13.10]字歌";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("<span begin=\"00:12.340\" end=\"00:12.800\">這是</span>"));
		assert!(ttml.contains("<span begin=\"00:12.800\" end=\"00:13.100\">逐</span>"));
		assert!(ttml.contains("<span begin=\"00:13.100\" end=\"60039.999\">字歌</span>"));
		// Crucially: spans must be back-to-back with no space text node.
		assert!(ttml.contains("</span><span begin=\"00:12.800\""));
		assert!(!ttml.contains("</span> <span"));
	}

	#[test]
	fn test_enhanced_lrc_with_spaces() {
		// Enhanced LRC with explicit spaces between words should emit ` `
		// text-nodes between spans (mirroring AMLL's `endsWithSpace`).
		let content = "[00:12.34] <00:12.34>Never <00:12.80>gonna <00:13.10>give <00:13.40>you <00:13.70>up";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("</span> <span"));
	}

	#[test]
	fn test_enhanced_lrc() {
		let content = "[00:12.34] <00:12.34>Never <00:12.80>gonna <00:13.10>give <00:13.40>you <00:13.70>up";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains(
			"<span begin=\"00:12.340\" end=\"00:12.800\">Never</span>"
		));
		assert!(ttml.contains(
			"<span begin=\"00:12.800\" end=\"00:13.100\">gonna</span>"
		));
		assert!(ttml.contains(
			"<span begin=\"00:13.100\" end=\"00:13.400\">give</span>"
		));
	}

	#[test]
	fn test_bilingual_lrc() {
		let content =
			"[00:12.34]Never gonna give you up\n[00:12.34]永远不会放弃你";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("Never gonna give you up"));
		assert!(ttml.contains("x-translation"));
		assert!(ttml.contains("永远不会放弃你"));
	}

	#[test]
	fn test_word_level_with_translation() {
		let content =
			"[00:12.34]Never[00:12.80]gonna\n[00:12.34]永远不会";
		let ttml = lrc_to_ttml(content);
		// Word-level line should have spans
		assert!(ttml.contains("<span begin=\"00:12.340\""));
		// Translation should be attached
		assert!(ttml.contains("x-translation"));
		assert!(ttml.contains("永远不会"));
	}

	#[test]
	fn test_metadata_tags() {
		let content =
			"[ti:Test Song]\n[ar:Test Artist]\n[al:Test Album]\n[00:12.34]Lyric text";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("musicName"));
		assert!(ttml.contains("Test Song"));
		assert!(ttml.contains("artists"));
		assert!(ttml.contains("Test Artist"));
		assert!(ttml.contains("album"));
		assert!(ttml.contains("Test Album"));
	}

	#[test]
	fn test_offset() {
		let content = "[offset:1000]\n[00:12.34]Lyric text";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("begin=\"00:13.340\""));
	}

	#[test]
	fn test_negative_offset() {
		let content = "[offset:-2000]\n[00:12.34]Lyric text";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("begin=\"00:10.340\""));
	}

	#[test]
	fn test_multi_timestamp() {
		let content = "[00:12.34][00:45.67]Same lyric at two points";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("begin=\"00:12.340\""));
		assert!(ttml.contains("begin=\"00:45.670\""));
		assert!(ttml.contains("Same lyric at two points"));
	}

	#[test]
	fn test_detect_ttml() {
		assert_eq!(
			detect_format("<?xml version=\"1.0\"?><tt></tt>"),
			LyricFormat::Ttml
		);
		assert_eq!(detect_format("<tt xmlns=\"...\"></tt>"), LyricFormat::Ttml);
	}

	#[test]
	fn test_detect_lrc() {
		assert_eq!(detect_format("[00:12.34]text"), LyricFormat::Lrc);
		assert_eq!(
			detect_format("[ti:Title]\n[00:12.34]text"),
			LyricFormat::Lrc
		);
	}

	#[test]
	fn test_detect_yrc() {
		assert_eq!(
			detect_format("[12340,1760](12340,460,0)Never"),
			LyricFormat::Yrc
		);
	}

	#[test]
	fn test_xml_escaping() {
		let content = "[00:12.34]This & that < or > \"text\"";
		let ttml = lrc_to_ttml(content);
		assert!(ttml.contains("This \x26amp; that \x26lt; or \x26gt; \x26quot;text\x26quot;"));
	}

	#[test]
	fn test_parse_lrc_timestamp() {
		assert_eq!(parse_lrc_timestamp("00:12.34"), Some(12340));
		assert_eq!(parse_lrc_timestamp("01:30.500"), Some(90500));
		assert_eq!(parse_lrc_timestamp("0:12"), Some(12000));
		assert_eq!(parse_lrc_timestamp("99:59.99"), Some(5999990));
	}

	#[test]
	fn test_is_timestamp() {
		assert!(is_timestamp("00:12.34"));
		assert!(is_timestamp("1:30.5"));
		assert!(!is_timestamp("ar:Artist"));
		assert!(!is_timestamp("offset:500"));
		assert!(!is_timestamp("ti:Title"));
	}

	#[test]
	fn test_format_time() {
		assert_eq!(format_time(12340), "00:12.340");
		assert_eq!(format_time(90500), "01:30.500");
		assert_eq!(format_time(0), "00:00.000");
		assert_eq!(format_time(SENTINEL_END_MS), "60039.999");
	}
}
