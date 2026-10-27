#!/usr/bin/env python3
"""Independent ONNX Runtime reference for Who encoders/queries logits."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np
import onnxruntime as ort

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from who_frontend import log_mel, parse_banks, reference_signal  # noqa: E402


def main() -> None:
    meta = json.loads((ROOT / "models/who/who_meta.json").read_text())
    window = next(w for w in meta["windows"] if w["frames"] == 30000)
    banks, mels, bins = parse_banks(ROOT / "models/who/who_mel_banks.f32")
    assert mels == 23 and bins == 257

    samples = reference_signal(100.0)
    mel, frames = log_mel(samples, banks)
    means = (mel.astype(np.float64).sum(axis=0) / frames).astype(np.float32)
    features = (mel - means).astype(np.float32)

    padded = np.empty((window["frames"], mels), dtype=np.float32)
    padded[:frames] = features
    padded[frames:] = features[-1]

    def session(name: str) -> ort.InferenceSession:
        return ort.InferenceSession(
            str(ROOT / "models/who" / name),
            providers=["CPUExecutionProvider"],
        )

    encoder = session(window["encoder"]["compiled"])
    memory, encoded_frames = encoder.run(
        None, {"mel": padded[None, :, :].astype(np.float32)}
    )

    inputs = {"memory": memory}
    at = 0
    for index, chunk in enumerate(window["chunks"]):
        inputs[f"frames_{index}"] = encoded_frames[:, at : at + chunk, :]
        at += chunk

    queries = session(window["queries"]["compiled"])
    outs = queries.run(None, inputs)
    names = [o.name for o in queries.get_outputs()]
    by_name = dict(zip(names, outs))
    presence = by_name["presence"].reshape(-1)
    query_vectors = by_name["queries"].reshape(-1)

    masks = np.zeros((meta["model"]["queries"], frames), dtype=np.float32)
    at = 0
    for index, chunk in enumerate(window["chunks"]):
        keep = min(chunk, frames - at)
        if keep <= 0:
            break
        chunk_masks = by_name[f"masks_{index}"][0]
        masks[:, at : at + keep] = chunk_masks[:, :keep]
        at += chunk

    spots = []
    for query in (0, 3, 7, 11):
        for frame in (0, 2500, 5000):
            spots.append(
                {
                    "query": query,
                    "frame": frame,
                    "value": float(masks[query, frame]),
                }
            )

    out = {
        "frames": int(frames),
        "presence": [float(v) for v in presence],
        "mask_row_means": [float(v) for v in masks.mean(axis=1)],
        "mask_spots": spots,
    }
    path = ROOT / "tests/fixtures/who_logits.json"
    path.write_text(json.dumps(out, indent=1) + "\n")
    print(f"wrote {path}: presence {presence.shape}, masks {masks.shape}")


if __name__ == "__main__":
    main()
