import json, sys
from pathlib import Path
base=Path(sys.argv[1]).resolve()
sys.path[:0]=[str(base/'deps'),str(base)]
import numpy,soundfile,cffi
print(json.dumps({'python':sys.version.split()[0],'numpy':numpy.__version__,'soundfile':soundfile.__version__,'libsndfile':soundfile.__libsndfile_version__,'cffi':cffi.__version__}))
