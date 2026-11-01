from decimal import Decimal, localcontext
from fractions import Fraction
import math,struct,json,hashlib
rows=[]
for precision in (80,160,240):
 with localcontext() as c:
  c.prec=precision
  ln2=Decimal(2).ln(); lo=-20*ln2; hi=4*ln2
  lower=float(lo); upper=float(hi)
  if Fraction.from_float(lower)<Fraction(lo): lower=math.nextafter(lower,math.inf)
  if Fraction.from_float(upper)>Fraction(hi): upper=math.nextafter(upper,-math.inf)
  rows.append({'precision':precision,'ln_lower':str(lo),'ln_upper':str(hi),'lower':lower.hex(),'lower_bits':f'{struct.unpack("<Q",struct.pack("<d",lower))[0]:016x}','upper':upper.hex(),'upper_bits':f'{struct.unpack("<Q",struct.pack("<d",upper))[0]:016x}','lower_inward_log_distance':str(Decimal.from_float(lower)-lo),'upper_inward_log_distance':str(hi-Decimal.from_float(upper)),'lower_ideal_exp_distance':str(lower and Decimal.from_float(lower).exp()-Decimal(2)**-20),'upper_ideal_exp_distance':str(Decimal(16)-Decimal.from_float(upper).exp())})
print(json.dumps(rows,indent=2))
