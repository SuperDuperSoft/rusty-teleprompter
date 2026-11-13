# Voice following v2 — on the word being said, never leaping

Date: 2026-11-12
Status: Built on request ("rethink the entire way we've been doing voice
following ... fix").

## Problems

1. On a long slide the highlight sometimes leaps far ahead.
2. The highlight always trails the voice by several words. The reader wants
   to see the word they are about to say and the next couple of words.

## Causes found (measured with `crates/core/tests/sim`)

- Leap: a repeated, unchanged partial counted as a second "vote", and the
  words that had already moved the position were matched again to a far
  copy of the same sentence. Reproduced as one 485-word step.
- Lag: the screen showed `tracker.position` (last confirmed word), and
  confirmation needed a later partial. Together with the recognizer's own
  delay this is 2–3 words behind, about 90% of the time.

## Design

Three layers, each with one job.

### 1. `tracker.rs` — what was heard (evidence)

`Tracker::accept` keeps its API. Inside:

- The whole utterance is aligned against the script from the **anchor**
  (the position when the utterance began). One monotone path places every
  heard word once: match, misheard, reader's own word, skipped script word,
  or one long jump (fixed cost plus a little per word).
- Long utterances settle their oldest words (40 rows kept, 20 settled at a
  time) and move the anchor to where they landed.
- An unchanged partial is not evidence. It only confirms two things that
  wait for a hypothesis to hold still: the slide's last word, and a move
  back.
- Committing: the next words commit at once (a match directly after the
  previous one, a match followed directly by another, or a distinctive
  word within reach). A run that starts after a gap of more than 3 script
  words must prove itself: 3 matches / 1 distinctive (gap ≤ 12),
  4 / 2 (≤ 45), 5 / 3 (more), dense; jumps over 12 words must also be seen
  again in a longer partial, or in a final.
- End of slide and going back keep their existing guards; going back is
  judged only on the heard words the script ahead could not explain.
- `seek`/`reset_utterance` void whatever the recognizer still holds of the
  current utterance.

### 2. `lead.rs` — where the reader is now (prediction)

`Session::cursor()` = next unheard word + pace × (recognizer delay + talking
time since the last heard word), at most 3 words ahead.

- Pace is measured from the heard words (recent, decaying), starting from
  the Speaking pace setting.
- At a sentence start it allows for this reader's usual pause, learned from
  how long the heard words rest there.
- Only while script words are being heard: speech that matched nothing does
  not move it; silence holds it; it never slides back while reading on;
  moving back (re-read, tap, retake) resets it.
- Setting **Highlight runs ahead (s)** (`highlight_lead`, default 0.6) is
  the assumed recognizer delay, since devices differ.

### 3. UI

The highlighted word and the scroll follow `cursor()`. Words before it are
dimmed; the next two are lifted (`soon-word`).

## Testing

- `tests/sim` — seeded reader + Vosk-like recognizer (late, repeating
  partials, wrong/dropped/extra words, unstable newest word, finals at
  pauses, optional skip / re-read / ad-lib).
- `tests/following.rs` — limits on cursor error, leaps, pace, skip, re-read,
  ad-lib.
- `tests/highlight.rs`, `tests/lead.rs` — the rules above, one by one,
  including the exact repeated-sentence leap.
- `tests/fuzz.rs` — never panics, never out of range.
- Existing suites (jumps, endings, resync, numbers, playback replay) pass;
  tests that pinned "wait for a repeat" were rewritten.

## Not verified here

Real recognizer delay on the S26, the A7 Lite and the PC. The setting covers
it; measuring speech onset natively to set it automatically is a possible
follow-up.
