#!/usr/bin/env python3
"""Independent NumPy reference for the Who log-mel frontend.

Mirrors the Swift implementation in Desert-Ant-Labs/desert-ant-core
(Sources/Who/Frontend.swift), not the Rust port, so it can act as a golden
oracle. Writes tests/fixtures/who_frontend.json.
"""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

import numpy as np

SAMPLE_RATE = 16_000
FRAME = 400
SHIFT = 160
N_FFT = 512
MELS = 23
PREEMPHASIS = np.float32(0.97)
SCALE = np.float32(32768.0)
LOG_FLOOR = np.float32(2.0**-23)

ROOT = Path(__file__).resolve().parents[2]


def parse_banks(path: Path) -> tuple[np.ndarray, int, int]:
    raw = path.read_bytes()
    mels, bins = struct.unpack_from("<II", raw, 0)
    assert len(raw) == 8 + 4 * mels * bins, (len(raw), mels, bins)
    data = np.frombuffer(raw, dtype="<f4", offset=8).reshape(mels, bins)
    return data, mels, bins


def reference_signal(seconds: float) -> np.ndarray:
    t = np.arange(int(SAMPLE_RATE * seconds), dtype=np.float64) / SAMPLE_RATE
    signal = (
        0.5 * np.sin(2.0 * np.pi * 440.0 * t)
        + 0.25 * np.sin(2.0 * np.pi * 1234.5 * t)
        + 0.05 * np.sin(2.0 * np.pi * 97.0 * t)
    )
    return signal.astype(np.float32)


def log_mel(samples: np.ndarray, banks: np.ndarray) -> tuple[np.ndarray, int]:
    frames = 1 + (len(samples) - FRAME) // SHIFT
    window = np.array(
        [0.54 - 0.46 * np.cos(2.0 * np.pi * n / (FRAME - 1)) for n in range(FRAME)],
        dtype=np.float64,
    ).astype(np.float32)

    mel = np.empty((frames, MELS), dtype=np.float32)
    for f in range(frames):
        start = f * SHIFT
        frame = (samples[start : start + FRAME] * SCALE).astype(np.float32)
        mean = np.float32(np.float64(frame).sum() / FRAME)
        frame = frame - mean
        # Pre-emphasis from the end, so each sample sees its unmodified predecessor.
        for i in range(FRAME - 1, 0, -1):
            frame[i] = np.float32(frame[i] - PREEMPHASIS * frame[i - 1])
        frame[0] = np.float32(frame[0] - PREEMPHASIS * frame[0])
        padded = np.zeros(N_FFT, dtype=np.float32)
        padded[:FRAME] = frame * window
        spectrum = np.fft.rfft(padded.astype(np.float64), n=N_FFT)
        power = (spectrum.real**2 + spectrum.imag**2).astype(np.float32)
        energies = (power.astype(np.float64) @ banks.T.astype(np.float64)).astype(np.float32)
        mel[f] = np.log(np.maximum(energies, LOG_FLOOR)).astype(np.float32)
    return mel, frames


def main() -> None:
    banks_path = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "models/who/who_mel_banks.f32"
    out_path = (
        Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "tests/fixtures/who_frontend.json"
    )
    banks, mels, bins = parse_banks(banks_path)
    assert (mels, bins) == (MELS, N_FFT // 2 + 1)

    samples = reference_signal(30.0)
    mel, frames = log_mel(samples, banks)
    means = (mel.astype(np.float64).sum(axis=0) / frames).astype(np.float32)
    features = (mel - means).astype(np.float32)

    spots = []
    for frame in (0, 499, 1000, 1500, 2000, 2500, 2997):
        for m in (0, 5, 11, 17, 22):
            spots.append(
                {
                    "mel": m,
                    "frame": frame,
                    "value": float(features[frame, m]),
                }
            )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        json.dumps(
            {
                "frames": int(frames),
                "means": [float(x) for x in means],
                "spots": spots,
            },
            indent=1,
        )
        + "\n"
    )
    print(f"wrote {out_path} ({frames} frames)")


if __name__ == "__main__":
    main()
