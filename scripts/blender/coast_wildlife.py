"""Original animated coastal wildlife, each below 800 triangles.

Shared by wildlife.py; no reference kit or external files are loaded.
"""
import math
import kit
import coast_common as c


def finish(name, parts, pivots, clips):
    mesh=kit.join(name,parts)
    bones=[('body',None,(0,0,0),(0,0,.2))]
    bones += [(bone,'body',point,(point[0],point[1],point[2]+.1)) for bone,point in pivots]
    arm=kit.armature(name+'_Rig',bones)
    kit.skin(arm,mesh)
    for clip,keys in clips.items():kit.action(arm,clip,16,keys,step=2)


def gull(p):
    parts=[kit.bind(kit.ball('Body',1,(0,0,.25),p['white'],segs=8,rings=4,scale=(.13,.25,.12)),'body'),
           kit.bind(kit.ball('Head',.085,(0,-.23,.32),p['white'],segs=8,rings=4),'body'),
           kit.bind(c.beam('Beak',(0,-.28,.31),(0,-.41,.31),.025,p['shell'],4),'body')]
    for side,bone in [(-1,'left'),(1,'right')]:
        outline=[(.10,-.06),(.26,-.13),(.49,.02),(.48,.12),(.25,.14)]
        parts.append(kit.bind(c.panel('Wing',[(side*x,y,.28) for x,y in outline],.018,p['white']),bone))
        tip=[(.49,.02),(.66,.12),(.60,.22),(.48,.12)]
        parts.append(kit.bind(c.panel('Wingtip',[(side*x,y,.28) for x,y in tip],.012,p['roof']),bone))
        parts.append(kit.bind(kit.ball('Eye',.012,(side*.055,-.278,.35),p['iron'],segs=6,rings=3),'body'))
        parts.append(kit.bind(kit.box('Tail',(.055,.2,.025),(side*.04,.24,.25),p['roof']), 'body'))
    finish('Gull',parts,[('left',(-.1,0,.28)),('right',(.1,0,.28))],{
        'idle':lambda t:{'body':(.02*math.sin(t*math.tau),0,0)},
        'flap':lambda t:{'left':(0,.6*math.sin(t*math.tau),0),'right':(0,-.6*math.sin(t*math.tau),0)},
        'glide':lambda t:{'left':(0,.12,0),'right':(0,-.12,0)},
    })


def crab(p):
    parts=[kit.bind(kit.ball('Shell',1,(0,0,.12),p['red'],segs=8,rings=4,scale=(.22,.16,.09)),'body')]
    for side,bone in [(-1,'left'),(1,'right')]:
        for i in range(4):
            y=(i-1.5)*.085
            parts.append(kit.bind(c.beam('Leg',(side*.17,y,.12),(side*.36,y+.08,.025),.016,p['red'],4),bone))
        parts.append(kit.bind(kit.ball('Claw',1,(side*.25,-.24,.15),p['red'],segs=6,rings=3,scale=(.055,.09,.05)),bone))
        parts.append(kit.bind(c.beam('EyeStalk',(side*.07,-.1,.15),(side*.07,-.14,.23),.014,p['iron'],4),'body'))
    finish('Crab',parts,[('left',(-.14,0,.1)),('right',(.14,0,.1))],{
        'idle':lambda t:{'body':(0,0,.02*math.sin(t*math.tau))},
        'walk':lambda t:{'left':(0,0,.18*math.sin(t*math.tau)),'right':(0,0,-.18*math.sin(t*math.tau))},
    })


def seal(p):
    parts=[kit.bind(kit.ball('Body',1,(0,0,.27),p['rock'],segs=10,rings=5,scale=(.3,.68,.27)),'body'),
           kit.bind(kit.ball('Head',1,(0,-.58,.4),p['rock'],segs=8,rings=4,scale=(.23,.25,.22)),'body'),
           kit.bind(kit.ball('Nose',.055,(0,-.80,.39),p['iron'],segs=6,rings=3),'body')]
    for side,bone in [(-1,'left'),(1,'right')]:
        parts.append(kit.bind(kit.ball('Flipper',1,(side*.3,-.22,.09),p['roof'],segs=6,rings=3,scale=(.28,.17,.05)),bone))
        parts.append(kit.bind(kit.ball('Eye',.024,(side*.13,-.75,.47),p['iron'],segs=6,rings=3),'body'))
        parts.append(kit.bind(kit.box('Tail',(.16,.25,.035),(side*.09,.65,.10),p['roof'],rot=(0,0,side*.3)),'body'))
    finish('Seal',parts,[('left',(-.22,-.22,.09)),('right',(.22,-.22,.09))],{
        'idle':lambda t:{'body':(.03*math.sin(t*math.tau),0,0)},
        'swim':lambda t:{'left':(0,.35*math.sin(t*math.tau),0),'right':(0,-.35*math.sin(t*math.tau),0)},
    })


def fish(p):
    parts=[kit.bind(kit.ball('Body',1,(0,0,.12),p['roof'],segs=8,rings=4,scale=(.045,.19,.09)),'body'),
           kit.bind(kit.box('Tail',(.018,.10,.17),(0,.21,.12),p['shell']),'tail'),
           kit.bind(kit.box('Dorsal',(.012,.11,.07),(0,0,.22),p['shell']),'body')]
    finish('Fish',parts,[('tail',(0,.14,.12))],{
        'idle':lambda t:{'tail':(0,0,.12*math.sin(t*math.tau))},
        'swim':lambda t:{'tail':(0,0,.4*math.sin(t*math.tau))},
    })


MODELS={'gull':gull,'crab':crab,'seal':seal,'fish':fish}


def build(folder,name):
    c.save(folder,name,MODELS[name],799)
