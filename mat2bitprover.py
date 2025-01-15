from math import *

def b(d11, d12, d21, d22):
    return \
        int((d11 + d12) != 0) * (2**3) + \
        int((d21 + d22) != 0) * (2**2) + \
        int((d11 + d21) != 0) * (2**1) + \
        int((d11 + d12 + d21 + d22) > 0) * (2**0)

tested = [0] * 16

for d11 in range(-1, 2):
    for d12 in range(-1, 2):
        for d21 in range(-1, 2):
            for d22 in range(-1, 2):

                if d11 != 0 and d12 != 0: continue
                if d21 != 0 and d22 != 0: continue
                if not ((d11 == 0 and d21 == 0) or (d12 == 0 and d22 == 0)): continue

                alld = [d11, d12, d21, d22]

                r = 0
                for d in alld: 
                    if d != 0: r = d

                skip=False
                for d in alld:
                    if d != 0 and d != r: skip=True

                if skip: continue

                i = b(d11, d12, d21, d22)
                if tested[i] != 0:
                    print("failed", d11, d12, d21, d22, "=", i)
                else:
                    print("ok", d11, d12, d21, d22, "=", i)
                    tested[i]=1

c = 0
for t in tested:
    c+=t

if c != 13:
    print("bad prover")
else:
    print("okay")