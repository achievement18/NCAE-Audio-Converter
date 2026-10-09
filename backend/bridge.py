"""Isolated multi-format branch bridge. No shell commands or global Python installation."""
import sys
from pathlib import Path
base=Path(__file__).resolve().parent
sys.path[:0]=[str(base/'deps'),str(base)]
import argparse,json,zipfile
import soundfile as sf
import ir_converter as converter
MAX_SAMPLES=16_777_216

def preflight(source):
    ext=source.suffix.lower()
    if ext=='.npz':
        with zipfile.ZipFile(source) as archive:
            if sum(item.file_size for item in archive.infolist()) > 256*1024*1024:
                raise ValueError('NPZ 解压后超过 256 MiB 安全上限')
    elif ext=='.json':
        if source.stat().st_size > 128*1024*1024:
            raise ValueError('JSON 超过 128 MiB 安全上限')
    else:
        info=sf.info(str(source))
        if info.channels < 1 or info.channels > 8 or info.frames < 1 or info.frames*info.channels > MAX_SAMPLES:
            raise ValueError('仅支持 1–8 声道、非空且不超过 16M 采样点的 IR')

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('source',type=Path)
    parser.add_argument('destination',type=Path)
    parser.add_argument('--subtype',default=None)
    args=parser.parse_args()
    try:
        source=args.source.resolve(); destination=args.destination.resolve()
        if source==destination: raise ValueError('源文件与输出文件不能相同')
        if destination.suffix.lower() not in set(converter.AUDIO_FORMATS) | {'.json','.npz'}: raise ValueError('不支持的输出格式')
        preflight(source)
        ir=converter.read_ir(source)
        if ir.samples.size>MAX_SAMPLES or ir.samples.shape[1]>8: raise ValueError('IR 超出采样数量或声道数上限')
        if ir.sample_rate>0xFFFFFFFF//(ir.samples.shape[1]*8): raise ValueError('采样率超出 WAV 容器范围')
        result=converter.write_ir(ir,destination,args.subtype)
        print(json.dumps(result,ensure_ascii=False))
    except (ValueError,OSError,KeyError,TypeError,zipfile.BadZipFile,sf.LibsndfileError) as error:
        print(str(error),file=sys.stderr)
        raise SystemExit(1)
