# Reference Environment and Feasibility Record

- **Status:** Pending G0 measurement
- **Owner:** Engineering owner
- **Governing story:** E0-S3
- **Decision output:** ADR-0002

## Environment inventory

| Field | Required value | Recorded value |
|---|---|---|
| Machine identifier | Stable nonsecret identifier | TBD |
| WSL version | `wsl --version` | TBD |
| Distribution | Ubuntu 24.04 identity | TBD |
| Kernel | `uname -a` | TBD |
| CPU model and topology | `lscpu` output/checksum | TBD |
| Visible physical/logical cores | Count and derivation | TBD |
| RAM and swap | Total and available | TBD |
| Storage and filesystem | Type, free space, root locations | TBD |
| Rust | `rustc`, Cargo, toolchain file | TBD |
| Python | Exact version and ABI | TBD |
| GCC/CMake | Exact versions | TBD |
| FFmpeg/ffprobe | Exact versions and build configuration | TBD |
| Chatterbox bundle | Source revision and bundle hash | TBD |
| Model/tokenizer/codec | URI, revision, checksum | TBD |
| Voice profile | Approved record ID and checksum | TBD |

## Qualification results

| Measurement | Gate | Result | Evidence ID |
|---|---:|---:|---|
| Offline real smoke render | Pass | TBD | TBD |
| Model load time | Record | TBD | TBD |
| Peak resident memory | Supports resource budget | TBD | TBD |
| Single-worker RTF at pool size one | `<= 6.0` | TBD | TBD |
| Projected 60-minute runtime | `<= 6 hours` | TBD | TBD |
| Worker output WAV compatibility | Pass | TBD | TBD |
| Fixed-seed ten-run byte identity | Characterize | TBD | TBD |
| Fixed-seed duration variance | Characterize | TBD | TBD |
| Acoustic similarity and listening | Characterize | TBD | TBD |
| Offline network observation | No runtime access | TBD | TBD |

## Root verification

Record resolved paths and prove that repository, target, Python environment, models, voices, cache, jobs, staging, and output are on the WSL2 Linux filesystem. Any `/mnt/c` runtime root fails G0.

## Reforecast and decision

Record the measured effect on M2 and M3. If any exit gate fails, stop backend implementation and reopen the hardware or backend decision before more work is committed.

