import sys, numpy as np
sys.path.insert(0, "/tmp/srl/art/src/mc")
from mcregion import Dim, extract_box
from collections import Counter
wd=sys.argv[1]; x0,z0,x1,z1,y0,y1=map(int,sys.argv[2:8])
pal, a = extract_box(Dim(f"{wd}/world/DIM-1/region"), x0,y0,z0,x1,y1,z1)
for y in range(a.shape[1]):
    c=Counter(pal[i].split('[')[0][10:] for i in a[:,y,:].ravel())
    print(y0+y, dict(c.most_common(4)))
