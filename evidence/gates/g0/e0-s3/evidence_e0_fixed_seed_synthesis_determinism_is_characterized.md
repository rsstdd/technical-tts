# Evidence Report: evidence_e0_fixed_seed_synthesis_determinism_is_characterized

- Governing story/gate: E0-S3 / G0
- Hypothesis or decision: Fixed-seed Chatterbox behavior is measured without treating a bounded
  sample as a byte-reconstruction guarantee
- Owner: Engineering owner
- Date/time and timezone: 2026-08-25 through 2026-08-26, Europe/Berlin
- Environment ID: `reference-wsl2-d9d550f06b783405`
- Status: Accepted

## Acceptance criterion

Stated before the result, per `evidence/README.md`: load the pinned model once in a namespace
with only loopback and no IP route, generate the identical approved input ten times with Python,
NumPy, and Torch reset to seed `42` immediately before each generation, and record every
container SHA-256, duration, RTF, media format, aligned waveform correlation, and log-mel cosine
similarity. Separate container metadata variation from decoded-PCM variation. A human reviewer
must assess all ten checksum-linked randomized outputs for omissions/additions, pronunciation,
voice consistency, pacing, noise, and audible differences before this evidence can be accepted.
No measured outcome changes the first-valid-artifact-wins cache rule or proves future
byte-identical reconstruction.

## Provenance

| Input | Identity/revision | URI | SHA-256 |
|---|---|---|---|
| Qualification input | `chatterbox-smoke-v1`, 69 UTF-8 bytes | `governed://technical-tts/e0-s3/qualification-input/v1` | `d4452958cea237afe27e574c4c8a9429fabe3a809e1be342d1749c6ac25266dc` |
| Chatterbox code | `v0.1.2`, commit `eb90621fa748f341a5b768aed0c0c12fc561894b` | Approved governed model root | Code-tree identity in worker SHA-256 `f5628884678f52de2f3a65ea51c9bc2a86e4f5919044fa9b4340eb62465dc2a9` |
| Model/tokenizer/codec | Revision `1b475dffa71fb191cb6d5901215eb6f55635a9b6` | Approved governed model root | Per-file checksums in `rights-chatterbox-weights-v2` and the raw result |
| Dependency inventory | Restored frozen environment | Approved governed environment root | `7de742701305fd95810a46bf575dc3c18377e5c910f9f48159f256f3e4af48e2` |
| Voice conditionals | `owner-fallback-v1` | Approved governed voice root | `f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00` |
| Qualification harness | `1.0-e0-s3-qualification` | `scripts/qualification/chatterbox_spike.py` | `67153661bd41b6e9b9c38b200265e324397baa70ab35c83e4354ab64e5fffa6a` |

The worker SHA-256 binds the acquired code tree and installed package, model revision and file
hashes, frozen dependencies, voice conditionals, generation parameters, CPU device, Python/ABI,
and three-intra-op/one-interop thread controls. The full redacted environment and decision
provenance is in `e0-s3-g0-qualification-report-v1.md` in this directory.

## Procedure

1. Verified the acquisition manifest, clean pinned code worktree, SafeTensor/tokenizer hashes,
   approved v2 rights records, consent, reference checksum, and conditionals checksum before
   model load.
2. Ran `/usr/bin/time -v unshare --user --map-root-user --net
   <qualification-python> scripts/qualification/chatterbox_spike.py` with governed local roots,
   seed `42`, run count `10`, three Torch threads, and one interop thread. The harness used only
   `ChatterboxTTS.from_local`, loaded the precomputed conditionals through upstream's
   weights-only load path, and reset all three RNGs before every generation.
3. Used repetition penalty `1.2`, minimum probability `0.05`, top probability `1.0`,
   exaggeration `0.5`, CFG weight `0.5`, and temperature `0.8`. Explicitly wrote mono 24 kHz
   32-bit IEEE-float WAV.
4. Aligned each decoded waveform to run one by maximum cross-correlation, then measured waveform
   correlation and 80-bin log-mel cosine similarity. Parsed and hashed every RIFF chunk and the
   decoded little-endian float samples separately.
5. Randomized checksum-preserving copies with a system RNG and sealed the source mapping in a
   separate key. Ross reviewed every blind sample sequentially through `ffplay` in the recorded
   WSL2/Framework environment before the key was opened. The completed review was published at
   a new immutable URI without replacing the generated pending sheet.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---:|---:|---|
| Network-isolated generation | Only loopback; no IP route | `lo` only; zero routes; offline flags set | Pass |
| Persistent model use | One load, 10 successful generations | One load; 10/10 completed | Pass |
| Output media | Mono, 24 kHz, 32-bit IEEE-float WAV | All ten matched | Pass |
| Container SHA-256 count | Characterize | 10 unique values | Recorded |
| Decoded-PCM SHA-256 count | Characterize | 1 unique value | Recorded |
| `data`-chunk SHA-256 count | Characterize | 1 unique value | Recorded |
| Varying RIFF chunks | Characterize | `PEAK` only; timestamp metadata | Recorded |
| Duration | Characterize | min 5.88 s; max 5.88 s; population standard deviation 0 | Recorded |
| Aligned waveform correlation | Characterize min/median/max | `1.0` / `1.0` / `1.0`; lag 0 | Recorded |
| Log-mel cosine similarity | Characterize min/median/max | `1.0` / `1.0` / `1.0` | Recorded |
| Blind-copy checksum validation | 10/10 copies match their recorded hashes | 10/10 passed `sha256sum -c` | Pass |
| Review/key one-to-one join | 10 blind IDs, hashes, and source runs; no duplicate | Both normalized `(blind_id, sha256)` sets have SHA-256 `68d247f7b5a2b495f19350315716629b5661b61164960fb7d0436862a86bde3e` | Pass |
| Ten-file randomized listening | Every file and criterion reviewed | 10/10 accepted; no omissions/additions, pronunciation, voice-consistency, pacing, noise/artifact, or audible-difference findings | Pass |

**Overall evidence acceptance: PASS.**

## Raw artifacts

| Artifact | Governed location | SHA-256 | Retention |
|---|---|---|---|
| Ten-run result | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/qualification-result-v1.json` | `3bfdb26348c240e5927d45e78c4de00e49d8babe759229b91f3668f9e538bddf` | While cited by G0 evidence or a superseding decision |
| Process measurement | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1-time.txt` | `574611e881c6af1c067b9e3e5af00fce393b5f698e2777dd25b16c03b79275bd` | Same |
| RIFF/PCM variation report | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/wav-variation-v1.json` | `12d81d1eb1f1094765f201d6413387c2c9a544c240c787870bbff54aec194fe9` | Same |
| Pending listening sheet | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/listening/review-sheet.json` | `5448f3fa9e8d55e0ff4b2b432b0b1390020cbea91e5c5c20a759ea570ad0a9ec` | Preserve unchanged; completed review receives a new immutable URI |
| Randomization key | `governed://technical-tts/e0-s3/2026-08-25/reference-wsl2-d9d550f06b783405/fixed-seed-ten-v1/listening/randomization-key.json` | `bae46560450fd5cfd0e918f95b697e1e7e383439024cdacd367eb3c560634cd7` | Opened only after all blind dispositions; retain with completed review |
| Completed listening review | `governed://technical-tts/e0-s3/2026-08-26/reference-wsl2-d9d550f06b783405/fixed-seed-ten-listening-review-v1/listening-review-v1.json` | `9ad19dba45dadb00de480c9493a981481cdce753a80f231f12c53d464e97d012` | Preserve unchanged while cited by this evidence |

## Interpretation and limitations

The ten WAV containers differ only because libsndfile records a `PEAK` timestamp. Their decoded
samples are identical in this environment and bounded run set. Stochastic behavior, numerical
libraries, a dependency change, hardware, or a future model version may still produce different
samples. The synthesis key therefore remains a request identity rather than a promise of bytes.
Retaining the selected cache artifact or archived segment bundle is the only recorded route to
byte-identical reconstruction.

The listener accepted all ten samples and reported no defect in any required category and no
audible difference between runs. This listening result characterizes only the reviewed output
set and does not change the reconstruction limitation above.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Recorded from automated measurements | Automated characterization complete | 2026-08-25 |
| Human reviewer | Ross | Accepted all ten blind samples | 2026-08-26 |
