// Run: node tests/lyrics-query.test.js
// Real titles as Windows reports them -> the query that must be among the candidates.
const assert = require("assert");
const path = require("path");
const { lyricsQueries, pickLyricsHit } = require(path.join(__dirname, "..", "ui", "lyrics-query.js"));

const cases = [
  ["Michael Jackson - Smooth Criminal (Official Video)", "michaeljacksonVEVO", "Smooth Criminal", "Michael Jackson"],
  ["Daft Punk - Get Lucky (Official Audio) ft. Pharrell Williams, Nile Rodgers", "Daft Punk", "Get Lucky", "Daft Punk"],
  ["Get Lucky (feat. Pharrell Williams)", "Daft Punk - Topic", "Get Lucky", "Daft Punk"],
  ["Queen – Bohemian Rhapsody (Official Video Remastered)", "Queen Official", "Bohemian Rhapsody", "Queen"],
  ["Bonnie Tyler - Total Eclipse of the Heart (Turn Around)", "BonnieTylerVEVO", "Total Eclipse of the Heart (Turn Around)", "Bonnie Tyler"],
  ["Shakira - Hips Don't Lie (Live) [HD]", "shakiraVEVO", "Hips Don't Lie (Live)", "Shakira"],
  ["Bad Bunny - DtMF (Video Oficial) | DeBÍ TiRAR MáS FOToS", "Bad Bunny", "DtMF", "Bad Bunny"],
  ["Rosalía - DESPECHÁ (Official Video)", "ROSALÍA", "DESPECHÁ", "Rosalía"],
  ["Numb (Official Music Video) [4K UPGRADE] – Linkin Park", "Linkin Park", "Numb", "Linkin Park"],
  ["Stairway to Heaven (Remaster)", "Led Zeppelin", "Stairway to Heaven", "Led Zeppelin"],
  ["Clean Bandit - Rockabye (feat. Sean Paul & Anne-Marie) [Official Video]", "Clean Bandit", "Rockabye", "Clean Bandit"],
  ["LA CANCIÓN (Letra/Lyrics)", "J Balvin, Bad Bunny", "LA CANCIÓN", "J Balvin, Bad Bunny"],
  ["AC/DC - Thunderstruck (Official Video)", "AC/DC", "Thunderstruck", "AC/DC"],
];
for (const [title, artist, track, wantArtist] of cases) {
  const qs = lyricsQueries(title, artist, "");
  assert.ok(qs.length <= 3, `too many queries for ${title}`);
  assert.deepStrictEqual(qs[0], { track: title, artist, album: "" }, "first query is what the player reported");
  assert.ok(qs.some((q) => q.track === track && q.artist === wantArtist), `${title} -> ${JSON.stringify(qs)}`);
}

// Song names that look like tags must survive untouched
assert.deepStrictEqual(lyricsQueries("Bailando con lobos", "Cabas", "").length, 1);
assert.deepStrictEqual(lyricsQueries("Smooth Criminal", "Michael Jackson", "").length, 1);

// Closest duration wins, plain-only results are ignored
const hit = pickLyricsHit([{ duration: 560, syncedLyrics: "a" }, { duration: 257, syncedLyrics: "b" }, { duration: 258, plainLyrics: "x" }], 258);
assert.strictEqual(hit.duration, 257);
assert.strictEqual(pickLyricsHit([{ duration: 200, plainLyrics: "x" }], 200), null);

console.log(`lyrics-query: ${cases.length + 4} checks ok`);
