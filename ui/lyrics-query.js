// =========================================================
// Lyrics query builder: turns what Windows reports for a track into cleaner LRCLIB queries.
// Video sites report things like
//   title  "Michael Jackson - Smooth Criminal (Official Video)"
//   artist "michaeljacksonVEVO"
// which LRCLIB never matches. Pure functions, no DOM: loaded before app.js and testable in
// Node (module.exports at the bottom).
// =========================================================

// A tag is upload noise (not part of the song name) when it IS one of these…
const LQ_NOISE_EXACT = [
  "video", "vídeo", "videoclip", "clip", "audio", "lyric", "lyrics", "letra", "letras",
  "mv", "m/v", "explicit", "clean", "visualizer", "visualiser", "subs", "vevo", "premiere",
  "estreno", "out now",
];
// …or CONTAINS one of these as whole words ("Official Music Video", "Remastered 2012")
const LQ_NOISE_WORDS = [
  "official", "oficial", "hd", "hq", "4k", "8k", "1080p", "720p", "music video", "lyric video", "video oficial", "audio oficial",
  "video musical", "videoclip oficial", "remaster", "remastered", "remasterizado",
  "remasterizada", "with lyrics", "con letra", "subtitulado", "subtitulada", "sub español",
  "traducida", "traducido", "traducción", "en español", "full album", "color coded",
  "performance video", "prod.", "prod by",
];

// Feature credits: dropped from the title for the search (LRCLIB lists them inconsistently).
// Not "con": it is also a plain word in Spanish titles ("Bailando con lobos").
const LQ_FEAT = /\s*[([]?\s*(?<![\p{L}\p{N}])(?:feat\.?|ft\.|featuring)\s+[^)\]]*[)\]]?/iu;

// Separators: "|" too for trailing tags, only dashes for "Artist - Song"
const LQ_DASH = /\s+[-–—|]\s+/;
const LQ_ARTIST_SEP = /\s+[-–—]\s+/;

function lqHasWord(text, word) {
  const esc = word.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
  return new RegExp(`(?<![\\p{L}\\p{N}])${esc}(?![\\p{L}\\p{N}])`, "iu").test(text);
}

function lqIsNoise(text) {
  const t = String(text || "").toLowerCase().trim();
  if (!t) return true;
  if (LQ_NOISE_EXACT.includes(t) || LQ_NOISE_WORDS.some((w) => lqHasWord(t, w))) return true;
  // Combined tags: "Letra/Lyrics", "Audio + Letra", "HD, Lyrics"
  const pieces = t.split(/\s*[/,+&]\s*/).filter(Boolean);
  return pieces.length > 1 && pieces.every((p) => p !== t && lqIsNoise(p));
}

// Remove "(Official Video)", "[Lyrics]", "【MV】", "| Official Audio"… keep "(Remix)", "(Live)"
function lqStripTags(title) {
  let s = String(title || "");
  // Bracketed groups whose content is noise
  s = s.replace(/\s*[([【「]([^)\]】」]*)[)\]】」]/g, (m, inner) => (lqIsNoise(inner) ? "" : m));
  // Trailing " - Official Video" / " | Lyrics" style tags (only if the tail is pure noise)
  for (;;) {
    const parts = s.split(LQ_DASH);
    if (parts.length < 2 || !lqIsNoise(parts[parts.length - 1])) break;
    s = parts.slice(0, -1).join(" - ");
  }
  s = s.replace(LQ_FEAT, "");
  return lqTidy(s);
}

function lqTidy(s) {
  return String(s || "")
    .replace(/["“”«»]/g, "")
    .replace(/\s{2,}/g, " ")
    .replace(/^[\s\-–—|:]+|[\s\-–—|:]+$/g, "")
    .trim();
}

// "michaeljacksonVEVO" -> "michaeljackson", "Daft Punk - Topic" -> "Daft Punk",
// "AdeleOfficial" -> "Adele". Camel case from VEVO channels is split ("TaylorSwift").
function lqCleanArtist(artist) {
  let a = String(artist || "").trim();
  const vevo = /vevo$/i.test(a);
  a = a.replace(/\s*-\s*topic$/i, "");
  a = a.replace(/vevo$/i, "");
  a = a.replace(/\s*(official|oficial)(\s+channel)?$/i, "");
  if (vevo && !/\s/.test(a)) a = a.replace(/([a-z])([A-Z])/g, "$1 $2");
  return lqTidy(a);
}

function lqNorm(s) {
  return String(s || "")
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]/g, "");
}

// Candidate queries, best guess first, without duplicates. The first one is always what
// Windows reported (exact match on LRCLIB's /get), the rest are cleaned-up variants.
function lyricsQueries(title, artist, album) {
  const rawTitle = String(title || "").trim();
  const rawArtist = String(artist || "").trim();
  const out = [];
  const seen = new Set();
  const add = (track, art, alb) => {
    track = lqTidy(track);
    art = lqTidy(art);
    if (!track) return;
    const key = `${lqNorm(track)}|${lqNorm(art)}`;
    if (seen.has(key)) return;
    seen.add(key);
    out.push({ track, artist: art, album: alb || "" });
  };

  add(rawTitle, rawArtist, album);

  // "Artist - Song | Album / show / channel": what follows the first "|" is not the song
  const beforeBar = rawTitle.split(/\s+\|\s+/)[0];
  const cleanTitle = lqStripTags(beforeBar.trim() ? beforeBar : rawTitle);
  const cleanArtist = lqCleanArtist(rawArtist);
  const na = lqNorm(cleanArtist);
  const sameArtist = (s) => {
    const n = lqNorm(s);
    return !!n && !!na && (n.includes(na) || na.includes(n));
  };

  // "Artist - Song" inside the title (video uploads)
  const parts = cleanTitle.split(LQ_ARTIST_SEP);
  if (parts.length >= 2) {
    const left = lqTidy(parts[0]);
    const right = lqStripTags(parts.slice(1).join(" - "));
    if (sameArtist(right) && !sameArtist(left)) {
      add(left, right, ""); // "Song - Artist"
    } else if (!na || sameArtist(left)) {
      add(right, left, ""); // "Artist - Song" (or the channel is a label: trust the title)
    } else {
      // Neither side names the channel: try both readings, the common one first
      add(right, left, "");
      add(left, right, "");
    }
  }
  // No "Artist - Song" split: the cleaned title with the cleaned channel name
  if (parts.length < 2) add(cleanTitle, cleanArtist, "");
  // Each query is a request to LRCLIB: the reported one plus two cleaned guesses at most
  return out.slice(0, 3);
}

// Pick the best synced result from an LRCLIB /search answer: closest duration wins when the
// player reported one (music videos run longer than the song, so it is a preference only)
function pickLyricsHit(results, durationSec) {
  if (!Array.isArray(results)) return null;
  const synced = results.filter((r) => r && typeof r.syncedLyrics === "string" && r.syncedLyrics);
  if (!synced.length) return null;
  const d = Number(durationSec) || 0;
  if (d <= 0) return synced[0];
  const close = synced.find((r) => Math.abs((Number(r.duration) || 0) - d) <= 3);
  return close || synced[0];
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = { lyricsQueries, pickLyricsHit, lqStripTags, lqCleanArtist };
}
