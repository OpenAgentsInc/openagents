"""Original surf-spray and sea-mist flipbooks from seeded analytic coverage.

Run Blender headless with -- OUTPUT_DIRECTORY [CELL]. The existing sheet
writer preserves premultiplied linear color. No renderer or lighting bake
runs; the sheet is computed on the CPU from droplets and soft density lobes.
"""
import hashlib
import json
import os
import sys
from pathlib import Path
import numpy as np
sys.path.insert(0,os.path.dirname(__file__))
import common

args=common.args();out=Path(args[0]);cell=int(args[1]) if len(args)>1 else 64
if not 16<=cell<=256:raise ValueError('Cell size must be 16 to 256')
out.mkdir(parents=True,exist_ok=True)
y,x=np.mgrid[-1:1:complex(cell),-1:1:complex(cell)]
rng=np.random.default_rng(738)
drops=rng.uniform([-.55,.6,.014],[.55,1.6,.045],size=(48,3))
for name in ['surf_spray','sea_mist']:
    frames=[]
    for frame in range(16):
        t=frame/15
        alpha=np.zeros((cell,cell),dtype=np.float32)
        if name=='surf_spray':
            for spread,speed,radius in drops:
                cx=spread*(.25+1.3*t)
                cy=.55-speed*t+1.15*t*t
                alpha=np.maximum(alpha,np.exp(-((x-cx)**2+(y-cy)**2)/(radius**2*2))*(1-t)**.6)
        else:
            for i in range(9):
                cx=.5*np.sin(i*2.4)+.1*t;cy=.28*np.cos(i*1.7)-.18*t
                radius=.12+.20*t
                alpha+=np.exp(-((x-cx)**2+(y-cy)**2)/(radius**2*2))*.15*(1-t)
        alpha=np.clip(alpha,0,1).astype(np.float32)
        rgba=np.zeros((cell,cell,4),dtype=np.float32)
        rgba[:,:,:3]=alpha[:,:,None]*np.array([.82,.91,1],dtype=np.float32)
        rgba[:,:,3]=alpha;frames.append(rgba)
    path=out/(name+'.png');common.write_sheet(str(path),frames,4,gain=1)
    record={'mode':'Reference','external_inputs':[],'frames':16,'columns':4,'cell':cell,
            'encoding':'premultiplied-linear-srgb','sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
    (out/(name+'.json')).write_text(json.dumps(record,indent=2)+'\n')
