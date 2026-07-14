#!/bin/bash
set -euo pipefail

# test-registration.sh — End-to-end daemon + registration smoke test
#
# Verifies:
#   1. AWS Lambda transport compiled in (hard dependency)
#   2. Daemon survives empty-secret startup (auth_loop retry)
#   3. start_registration RPC returns instantly (get_puzzle in background)
#   4. Registration makes progress / completes
#
# Usage:  ./test-registration.sh [path/to/geph5-client]

BINARY="${1:-target/release/geph5-client}"
DAEMON_LOG="/tmp/geph-test-daemon.log"
TEST_CONFIG="/tmp/geph-test-config.yaml"
TEST_CACHE="/tmp/geph-test-cache.db"

CTRL_PORT=13222
SOCKS_PORT=19009
HTTP_PORT=19110
PAC_PORT=13223
TIMEOUT_REG=60

PASS=0; FAIL=0
ok()   { printf '\033[1;32m[PASS]\033[0m %s\n' "$*"; PASS=$((PASS+1)); }
fail() { printf '\033[1;31m[FAIL]\033[0m %s\n' "$*"; FAIL=$((FAIL+1)); }
info() { printf '\033[1;33m      \033[0m %s\n' "$*"; }

cleanup() {
    [ -n "${DPID:-}" ] && kill -9 "$DPID" 2>/dev/null || true
    [ -n "${DPID:-}" ] && wait "$DPID" 2>/dev/null || true
    rm -f "$TEST_CONFIG" "$TEST_CACHE"* "$DAEMON_LOG"
}
trap cleanup EXIT

[ -f "$BINARY" ] || { echo "Binary not found: $BINARY"; exit 1; }
echo "Binary: $BINARY"
echo ""

# ── 1. Symbol check ────────────────────────────────────
echo "=== 1. Binary symbol check ==="
if grep -q "calling broker through lambda" "$BINARY" 2>/dev/null; then
    ok "AWS Lambda transport compiled in"
else
    fail "AWS Lambda NOT in binary"
fi
echo ""

# ── 2. Config ──────────────────────────────────────────
cat > "$TEST_CONFIG" << CFGEOF
socks5_listen: 127.0.0.1:$SOCKS_PORT
http_proxy_listen: 127.0.0.1:$HTTP_PORT
pac_listen: 127.0.0.1:$PAC_PORT
control_listen: 127.0.0.1:$CTRL_PORT
exit_constraint: auto
allow_direct: false
cache: $TEST_CACHE
broker:
  priority_race:
    1500:
      aws_lambda:
        function_name: geph-lambda-bouncer
        region: us-east-1
        obfs_key: "855MJGAMB58MCPJBB97NADJ36D64WM2T:C4TN2M1H68VNMRVCCH57GDV2C5VN6V3RB8QMWP235D0P4RT2ACV7GVTRCHX3EC37"
    300:
      fronted:
        front: https://kubernetes.io/
        host: svitania-naidallszei-2.netlify.app
        override_dns:
          - 75.2.60.5:443
    0:
      fronted:
        front: https://kubernetes.io/
        host: svitania-naidallszei-2.netlify.app
tunneled_broker:
  direct: https://broker.geph.io
broker_keys:
  master: 88c1d2d4197bed815b01a22cadfc6c35aa246dddb553682037a118aebfaa3954
  mizaru_free: 0558216cbab7a9c46f298f4c26e171add9af87d0694988b8a8fe52ee932aa754
  mizaru_plus: cf6f58868c6d9459b3a63bc2bd86165631b3e916bad7f62b578cd9614e0bcb3b
  mizaru_bw: 3082010a0282010100d0ae53a794ea37bf2e100cb3a872177ec6c11e8375fdcbf92960ce0293465674eb1426a1841b7622a58979a5ff3f8aa2301a621545e9b90bb39d1a6bfda19d6ca1aae74a3192ddfd2b9558eb652c3c2c22f42bdde272852fb67d93cae5846213512c474bf799844aee019bf718f6fa64223be06364459fc8dec66796b141d450d730c4fffe1cac7df8f05591560afa44bcf274f6c0e2303b39c21ab09d19b459ee594512b8341f3d407c026e2509f42c6d89f82f6a3a36fd5c05ad423cd99ad39089403eb9122ea60ef6648afff65438e8e26ce41fa55b9b18741965c77a627bae947bd38fc345e9adab42d6c458f6e194e4232cfd3f04924d5a5e932fe769610203010001
port_forward: []
vpn: false
vpn_fd: null
spoof_dns: false
passthrough_china: false
dry_run: false
credentials:
  secret: ""
sess_metadata: {}
task_limit: null
CFGEOF

# ── 3. Start daemon ────────────────────────────────────
echo "=== 2. Start daemon (empty secret) ==="
RUST_LOG=debug "$BINARY" --config "$TEST_CONFIG" > "$DAEMON_LOG" 2>&1 &
DPID=$!

for i in $(seq 1 120); do
    bash -c "exec 3<>/dev/tcp/127.0.0.1/$CTRL_PORT" 2>/dev/null && { exec 3>&- 3<&-; break; }
    sleep 0.25
done

kill -0 "$DPID" 2>/dev/null && ok "Daemon alive" || { fail "Daemon died"; tail -10 "$DAEMON_LOG"; exit 1; }
echo ""

# ── 4. Survival ────────────────────────────────────────
echo "=== 3. Survival (10s) ==="
sleep 10
kill -0 "$DPID" 2>/dev/null && ok "Survived 10s" || { fail "Crashed <10s"; tail -10 "$DAEMON_LOG"; exit 1; }
grep -q "retrying" "$DAEMON_LOG" && ok "auth_loop retrying" || fail "no auth retry in log"
echo ""

# ── 5. RPC helper ──────────────────────────────────────
rpc() {
    python3 -c "
import socket, sys
s = socket.socket()
s.settimeout(5)
s.connect(('127.0.0.1', $CTRL_PORT))
s.sendall((sys.argv[1] + '\n').encode())
d = b''
while True:
    try:
        c = s.recv(4096)
        if not c: break
        d += c
        if b'\n' in d: break
    except: break
s.close()
sys.stdout.write(d.decode().strip())
" "$1"
}

# ── 6. Registration RPC ────────────────────────────────
echo "=== 4. Registration RPC ==="
T0=$(date +%s%N)
RESP=$(rpc '{"jsonrpc":"2.0","method":"start_registration","params":[],"id":1}')
T1=$(date +%s%N)
MS=$(( (T1-T0)/1000000 ))

if echo "$RESP" | grep -q '"result"'; then
    ok "start_registration ok (${MS}ms): $RESP"
else
    fail "start_registration fail: ${RESP:-<empty>}"
fi
if [ "$MS" -lt 3000 ]; then ok "Fast (<3s)"; else fail "Slow (${MS}ms)"; fi
echo ""

# ── 7. Progress ────────────────────────────────────────
echo "=== 5. Progress (${TIMEOUT_REG}s) ==="
SECRET=""
for i in $(seq 1 $((TIMEOUT_REG/5))); do
    sleep 5
    P=$(rpc '{"jsonrpc":"2.0","method":"poll_registration","params":[0],"id":2}')
    if echo "$P" | grep -q '"secret":"[^"]'; then
        SECRET=$(echo "$P" | python3 -c "import sys,json;print(json.load(sys.stdin)['result']['secret'])" 2>/dev/null)
        ok "Registered! secret=${SECRET:0:8}..."
        break
    elif echo "$P" | grep -q '"progress"'; then
        PCT=$(echo "$P" | python3 -c "import sys,json;d=json.load(sys.stdin)['result'];print(f'{d[\"progress\"]*100:.1f}%')" 2>/dev/null)
        info "$PCT"
    else
        info "poll: ${P:-<empty>}"
    fi
done

if [ -n "$SECRET" ]; then
    ok "Account registered"
else
    fail "Stalled"
    info "Broker activity:"
    grep -iE "lambda|puzzle|restart" "$DAEMON_LOG" | tail -5 | sed 's/^/      /'
fi
echo ""

# ── 8. Broker transport ────────────────────────────────
echo "=== 6. Broker transport ==="
if grep -qE "aws_sdk_lambda|lambda.us-east-1" "$DAEMON_LOG"; then
    ok "AWS Lambda transport active"
else
    fail "No Lambda activity"
fi
if grep -q "got puzzle" "$DAEMON_LOG"; then
    ok "Puzzle received from broker"
else
    info "No puzzle yet (broker unreachable or slow)"
fi
echo ""

# ── Summary ────────────────────────────────────────────
echo "========================================"
printf '  \033[1;32m%d passed\033[0m' "$PASS"
if [ "$FAIL" -gt 0 ]; then printf '  \033[1;31m%d failed\033[0m' "$FAIL"; fi
echo ""
echo "========================================"
exit $([ "$FAIL" -eq 0 ] && echo 0 || echo 1)
