from decimal import Decimal,localcontext
from fractions import Fraction
with localcontext() as c:
 c.prec=90
 pi=Decimal('3.141592653589793238462643383279502884197169399375105820974944592307816406286208998628034825')
 x=(2*pi).ln()/2
 f=float(x)
 print('C exact decimal',x)
 print('nearest C',repr(f),f.hex(),Decimal.from_float(f)-x)
 print('frozen literal',float('0.9189385332046727').hex(),Decimal.from_float(float('0.9189385332046727'))-x)
 print('TWO_PI',repr(float(2*pi)),float(2*pi).hex())
