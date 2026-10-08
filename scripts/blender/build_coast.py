"""Build only the original coast families in one headless Blender process.

Run with -- OUTPUT_DIRECTORY [FAMILY ...]. No rendering or lighting bake.
"""
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0,os.path.dirname(__file__))
import kit
import coast_common as c
import coast_rocks,beach_props,harbor,lighthouse,boats,reef,coast_wildlife

args=kit.args()
root=args[0] if args else os.path.join(kit.REPO,'assets','verse','generated','coast')
families={'rocks':coast_rocks,'beach':beach_props,'harbor':harbor,'lighthouse':lighthouse,'boats':boats,'reef':reef}
for family in args[1:] or list(families)+['wildlife']:
    folder=os.path.join(root,family)
    if family=='wildlife':
        for name in coast_wildlife.MODELS:coast_wildlife.build(folder,name)
    else:
        for name,(build,budget) in families[family].MODELS.items():c.save(folder,name,build,budget)
