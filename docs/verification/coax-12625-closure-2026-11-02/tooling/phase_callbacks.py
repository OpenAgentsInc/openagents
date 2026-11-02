# Firm reason: 2 (independent read-only LLDB measurement of actual compiler counters).
import json,lldb
pending={}

def scalar(value,name):
 v=value.GetChildMemberWithName(name)
 return v.GetValueAsSigned() if v.IsValid() and not v.GetError().Fail() else None

def state_value(value):
 return value.GetChildMemberWithName('state').GetChildMemberWithName('data').Dereference()

def counters(value):return {key:scalar(value,key) for key in ('queries','origins','work','depth')}

def emit(row):print('COAX_PHASE '+json.dumps(row,sort_keys=True),flush=True)

def capture(frame,bp_loc,internal_dict):
 name=frame.GetFunctionName();context=frame.FindVariable('l_b') if name.endswith('CR_run') else frame.FindVariable('l_r').GetChildMemberWithName('b')
 row={'event':'phase-entry','function':name,'counters':counters(state_value(context))}
 if name.endswith('CR_walk'):
  row['seed']=frame.FindVariable('l_seed').GetValue();row['instances']=frame.FindVariable('l_instances').GetValue()
 else:
  # Save a native pointer/type, not a lazy value tied to the callee's expired frame.
  ptr=context.GetChildMemberWithName('state').GetChildMemberWithName('data');caller=frame.GetThread().GetFrameAtIndex(1);address=ptr.GetValueAsUnsigned();return_pc=caller.GetPC();kind=ptr.GetType().GetPointeeType()
  if ptr.IsValid() and not ptr.GetError().Fail() and address and kind.IsValid() and caller.IsValid() and return_pc!=lldb.LLDB_INVALID_ADDRESS:
   target=frame.GetThread().GetProcess().GetTarget();bp=target.BreakpointCreateByAddress(return_pc);bp.SetOneShot(True);bp.SetThreadID(frame.GetThread().GetThreadID());bp.SetScriptCallbackFunction('phase_callbacks.returned');pending[bp.GetID()]=(address,kind)
   row['return_watch']='registered'
  else:row['return_watch']='unavailable'
 emit(row);return False

def returned(frame,bp_loc,internal_dict):
 item=pending.pop(bp_loc.GetBreakpoint().GetID(),None);row={'event':'CR_run-return','counters':None}
 if item:
  address,kind=item;target=frame.GetThread().GetProcess().GetTarget();value=target.CreateValueFromAddress('coax_returned_state',lldb.SBAddress(address,target),kind)
  if value.IsValid() and not value.GetError().Fail():row['counters']=counters(value)
 emit(row);return False

def finish(debugger):
 p=debugger.GetSelectedTarget().GetProcess();row={'event':'finish','state':p.GetState(),'exited':p.GetState()==lldb.eStateExited,'exit_status':p.GetExitStatus()}
 if p.GetState()==lldb.eStateStopped:
  thread=p.GetSelectedThread();row['frames']=[str(frame) for frame in thread];frame=thread.GetFrameAtIndex(1);b=frame.FindVariable('l_b');row['counters']=counters(state_value(b)) if b.IsValid() else None
 emit(row)
