#!/bin/sh
# Create a small test STL
python3 -c '
import base64, struct
h = b"\0"*80
c = struct.pack("<I",1)
t = struct.pack("<12fH",0,0,1, 0,0,0, 1,0,0, 0,1,0, 0)
with open("/tmp/t.stl","wb") as f: f.write(h+c+t)
print(base64.b64encode(open("/tmp/t.stl","rb").read()).decode())
' > /tmp/tb64.txt
B64=$(cat /tmp/tb64.txt)
TOKEN=$(curl -s -X POST http://localhost:9090/api/auth/login \
  -H "Content-Type:application/json" \
  -d @/tmp/login.json | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")
echo "=== Testing POST /api/cases/upload ==="
curl -v -X POST http://localhost:9090/api/cases/upload \
  -H "Content-Type:application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{\"name\":\"debug-test\",\"solver\":\"simpleFoam\",\"flow_type\":\"incompressible\",\"stl_data\":\"$B64\",\"stl_filename\":\"wing.stl\"}" 2>&1
