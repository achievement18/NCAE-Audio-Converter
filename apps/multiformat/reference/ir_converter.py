"""Bidirectional IR conversion. Dependencies: numpy, soundfile.

Audio: WAV/IRS/FLAC/AIFF/AU/CAF/W64/RF64.
Data: JSON (ir.samples.v1 only), NPZ (ir.samples.v1 only).
Preserves sample rate, frame count and channel order. No DSP or resampling.
IRS output is a WAV container, not a proprietary IRS codec.
"""
from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import soundfile as sf

SCHEMA = "ir.samples.v1"
AUDIO_FORMATS = {
    ".wav": "WAV", ".wave": "WAV", ".irs": "WAV",
    ".flac": "FLAC", ".aif": "AIFF", ".aiff": "AIFF",
    ".au": "AU", ".snd": "AU", ".caf": "CAF",
    ".w64": "W64", ".rf64": "RF64",
}
PCM_BITS = {"PCM_16": 16, "PCM_24": 24, "PCM_32": 32}
ENCODINGS = set(PCM_BITS) | {"FLOAT", "DOUBLE"}


@dataclass
class IR:
    samples: np.ndarray  # [frames, channels], normalized float64
    sample_rate: int
    source_subtype: str = "DOUBLE"

    def validate(self) -> None:
        if isinstance(self.sample_rate, bool) or not isinstance(
            self.sample_rate, (int, np.integer)
        ) or self.sample_rate <= 0:
            raise ValueError("sample_rate must be a positive integer")
        self.samples = np.asarray(self.samples, dtype=np.float64)
        if self.samples.ndim != 2 or min(self.samples.shape) == 0:
            raise ValueError("samples must be a non-empty [frames, channels] array")
        if not np.isfinite(self.samples).all():
            raise ValueError("IR contains NaN or Infinity")
        if not isinstance(self.source_subtype, str):
            raise ValueError("source_subtype must be a string")


def read_ir(path: str | Path) -> IR:
    path = Path(path)
    ext = path.suffix.lower()
    if ext == ".json":
        obj = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(obj, dict) or obj.get("schema") != SCHEMA:
            raise ValueError(
                "Only ir.samples.v1 JSON is supported; arbitrary EQ/ViPER "
                "presets need a software-specific adapter"
            )
        ir = IR(obj["samples"], obj["sample_rate"],
                obj.get("source_subtype", "DOUBLE"))
    elif ext == ".npz":
        with np.load(path, allow_pickle=False) as obj:
            if obj["schema"].item() != SCHEMA:
                raise ValueError("Unsupported NPZ schema")
            ir = IR(obj["samples"], obj["sample_rate"].item(),
                    obj["source_subtype"].item())
    elif ext in AUDIO_FORMATS:
        try:
            with sf.SoundFile(str(path)) as audio:
                ir = IR(audio.read(dtype="float64", always_2d=True),
                        audio.samplerate, audio.subtype)
        except sf.LibsndfileError as exc:
            raise ValueError(
                "Cannot decode audio. A proprietary IRS file is not "
                "automatically convertible by renaming it."
            ) from exc
    else:
        raise ValueError(f"Unsupported input extension: {ext}")
    ir.validate()
    return ir


def select_encoding(ir: IR, container: str, subtype: str | None) -> str:
    if subtype is not None:
        encoding = subtype.upper()
    elif ir.source_subtype in ENCODINGS and sf.check_format(
        container, ir.source_subtype
    ):
        encoding = ir.source_subtype
    else:
        encoding = "PCM_24" if container == "FLAC" else "DOUBLE"

    if encoding not in ENCODINGS or not sf.check_format(container, encoding):
        raise ValueError(f"Unsupported container/encoding: {container}/{encoding}")

    if encoding in PCM_BITS:
        maximum = 1.0 - 2.0 ** (1 - PCM_BITS[encoding])
        if np.any(ir.samples < -1.0) or np.any(ir.samples > maximum):
            raise ValueError(
                f"Samples exceed {encoding} range; use floating-point WAV "
                "or explicitly scale the IR yourself"
            )
    elif encoding == "FLOAT" and np.any(
        np.abs(ir.samples) > np.finfo(np.float32).max
    ):
        raise ValueError("Samples exceed FLOAT range; use DOUBLE")
    return encoding


def write_ir(ir: IR, path: str | Path, subtype: str | None = None) -> dict:
    ir.validate()
    path = Path(path)
    ext = path.suffix.lower()
    container = AUDIO_FORMATS.get(ext)
    if container is None and ext not in {".json", ".npz"}:
        raise ValueError(f"Unsupported output extension: {ext}")
    if container is None and subtype is not None:
        raise ValueError("subtype only applies to audio output")
    encoding = select_encoding(ir, container, subtype) if container else None

    # Exclusive creation: never overwrite existing output.
    output = path.open("xb")
    try:
        with output:
            if ext == ".json":
                obj = {
                    "schema": SCHEMA,
                    "sample_rate": int(ir.sample_rate),
                    "source_subtype": ir.source_subtype,
                    "samples": ir.samples.tolist(),
                }
                output.write(json.dumps(obj, ensure_ascii=False,
                                        allow_nan=False).encode("utf-8"))
            elif ext == ".npz":
                np.savez_compressed(
                    output, schema=SCHEMA, samples=ir.samples,
                    sample_rate=int(ir.sample_rate),
                    source_subtype=ir.source_subtype,
                )
            else:
                sf.write(output, ir.samples, int(ir.sample_rate),
                         format=container, subtype=encoding)
    except Exception:
        path.unlink(missing_ok=True)  # Remove only our newly created output.
        raise

    return {
        "path": str(path.resolve()),
        "format": container or ext[1:].upper(),
        "subtype": encoding,
        "source_subtype": ir.source_subtype,
        "encoding_changed": encoding is not None and encoding != ir.source_subtype,
        "sample_rate": int(ir.sample_rate),
        "frames": ir.samples.shape[0],
        "channels": ir.samples.shape[1],
    }


def convert(source: str | Path, destination: str | Path,
            *, subtype: str | None = None) -> dict:
    """Convert either direction, inferred from destination extension.

    Audio metadata/chunks are not copied. The sample payload, sample rate,
    and channel order are preserved except for output-encoding quantization.
    JSON/NPZ are our sample-data schema, NOT software effect presets.
    """
    src = Path(source).expanduser().resolve()
    dst = Path(destination).expanduser().resolve()
    if not src.is_file():
        raise FileNotFoundError(src)
    if src == dst:
        raise ValueError("Input and output must be different files")
    return write_ir(read_ir(src), dst, subtype)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source")
    parser.add_argument("destination")
    parser.add_argument("--subtype", choices=sorted(ENCODINGS))
    args = parser.parse_args()
    try:
        print(json.dumps(convert(args.source, args.destination,
                                 subtype=args.subtype), indent=2))
    except (ValueError, OSError, KeyError, sf.LibsndfileError) as exc:
        parser.exit(1, f"Conversion failed: {exc}\n")
