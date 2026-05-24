#!/bin/sh
TOKEN=$(curl -s -X POST http://localhost:9090/api/auth/login \
  -H "Content-Type:application/json" \
  -d @/tmp/login.json | sed 's/.*"token":"\([^"]*\)".*/\1/')
echo '{"name":"test2","solver":"simpleFoam","flow_type":"incompressible"}' > /tmp/create2.json
curl -s -X POST http://localhost:9090/api/cases \
  -H "Content-Type:application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d @/tmp/create2.json
